use crate::{
    audio::{AudioBackend, AudioCommand, AudioEvent},
    error::{Error, Result},
    models::{Availability, StreamSource, Track},
    providers::ProviderRegistry,
    queue::{Queue, RepeatMode},
    storage::Storage,
};
use serde::Serialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    Stopped,
    Loading,
    Buffering,
    Playing,
    Paused,
    Error,
}
#[derive(Debug, Clone, Serialize)]
pub struct PlayerState {
    pub status: PlaybackStatus,
    pub current: Option<Track>,
    pub entry_id: Option<Uuid>,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub error: Option<String>,
}
#[derive(Clone)]
pub enum PlayerCommand {
    Play { tracks: Vec<Track>, index: usize },
    Select(Uuid),
    Enqueue { tracks: Vec<Track>, next: bool },
    Remove(Uuid),
    Reorder { entry_id: Uuid, to: usize },
    Clear,
    Toggle,
    Pause,
    Resume,
    Stop,
    Seek(u64),
    Volume(f64),
    Next,
    Previous,
    Shuffle(bool),
    Repeat(RepeatMode),
    Device(String),
    Shutdown,
}
enum Message {
    Action(PlayerCommand, oneshot::Sender<Result<()>>),
    Resolved {
        generation: u64,
        result: Result<StreamSource>,
    },
}
#[derive(Clone)]
pub struct Player {
    sender: mpsc::Sender<Message>,
    pub state: watch::Receiver<PlayerState>,
    pub queue: watch::Receiver<Queue>,
}
impl Player {
    pub fn start(
        storage: Storage,
        providers: ProviderRegistry,
        audio: Arc<dyn AudioBackend>,
        events: mpsc::UnboundedReceiver<AudioEvent>,
    ) -> Result<Self> {
        let settings = storage.settings()?;
        let (queue, position) = storage.load_queue()?;
        let current = queue.current().map(|e| e.track.clone());
        let state = PlayerState {
            status: if current.is_some() {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Stopped
            },
            duration_ms: current.as_ref().map(|t| t.duration_ms).unwrap_or(0),
            current,
            entry_id: queue.current,
            position_ms: position,
            volume: settings.volume,
            shuffle: queue.shuffle,
            repeat: queue.repeat,
            error: None,
        };
        audio.send(AudioCommand::Volume(settings.volume))?;
        audio.send(AudioCommand::Device(settings.audio_device))?;
        let (sender, receiver) = mpsc::channel(128);
        let (state_tx, state_rx) = watch::channel(state.clone());
        let (queue_tx, queue_rx) = watch::channel(queue.clone());
        let actor = Actor {
            storage,
            providers,
            audio,
            queue,
            state,
            state_tx,
            queue_tx,
            sender: sender.clone(),
            generation: 0,
            resolver: None,
            loaded: false,
            want_play: false,
            retries: 0,
            failed: 0,
            last_save: Instant::now(),
            history_recorded: false,
        };
        tokio::spawn(actor.run(receiver, events));
        Ok(Self {
            sender,
            state: state_rx,
            queue: queue_rx,
        })
    }
    pub async fn command(&self, command: PlayerCommand) -> Result<()> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Message::Action(command, sender))
            .await
            .map_err(|_| Error::Audio("Player stopped".into()))?;
        receiver
            .await
            .map_err(|_| Error::Audio("Player stopped".into()))?
    }
}
struct Actor {
    storage: Storage,
    providers: ProviderRegistry,
    audio: Arc<dyn AudioBackend>,
    queue: Queue,
    state: PlayerState,
    state_tx: watch::Sender<PlayerState>,
    queue_tx: watch::Sender<Queue>,
    sender: mpsc::Sender<Message>,
    generation: u64,
    resolver: Option<tokio::task::JoinHandle<()>>,
    loaded: bool,
    want_play: bool,
    retries: u8,
    failed: usize,
    last_save: Instant,
    history_recorded: bool,
}
impl Actor {
    async fn run(
        mut self,
        mut messages: mpsc::Receiver<Message>,
        mut events: mpsc::UnboundedReceiver<AudioEvent>,
    ) {
        loop {
            tokio::select! {
                Some(message)=messages.recv()=>match message {
                    Message::Action(command,reply)=>{let shutdown=matches!(command,PlayerCommand::Shutdown);let result=self.action(command);if let Err(e)=&result{tracing::warn!(error=%e,"player command rejected");}let _=reply.send(result);if shutdown{break;}},
                    Message::Resolved{generation,result} if generation==self.generation=>self.resolved(result),
                    Message::Resolved{..}=>{},
                },
                Some(event)=events.recv()=>self.event(event),
                else=>break,
            }
        }
        if let Some(resolver) = self.resolver.take() {
            resolver.abort();
        }
        let _ = self.save();
        let _ = self.audio.send(AudioCommand::Shutdown);
    }
    fn action(&mut self, command: PlayerCommand) -> Result<()> {
        let mut queue_changed = false;
        match command {
            PlayerCommand::Play { tracks, index } => {
                self.queue.replace(tracks, index)?;
                self.failed = 0;
                self.begin(0, true, true)?;
                queue_changed = true;
            }
            PlayerCommand::Select(id) => {
                self.queue.select(id)?;
                self.failed = 0;
                self.begin(0, true, true)?;
                queue_changed = true;
            }
            PlayerCommand::Enqueue { tracks, next } => {
                self.queue.enqueue(tracks, next)?;
                queue_changed = true;
            }
            PlayerCommand::Remove(id) => {
                if self.queue.remove(id)? {
                    if self.queue.current.is_some() {
                        self.begin(0, self.want_play, true)?;
                    } else {
                        self.stop()?;
                    }
                }
                queue_changed = true;
            }
            PlayerCommand::Reorder { entry_id, to } => {
                self.queue.reorder(entry_id, to)?;
                queue_changed = true;
            }
            PlayerCommand::Clear => {
                self.queue.clear();
                self.stop()?;
                self.state.current = None;
                self.state.entry_id = None;
                self.state.duration_ms = 0;
                queue_changed = true;
            }
            PlayerCommand::Toggle => {
                if self.want_play {
                    self.pause()?;
                } else {
                    self.resume()?;
                }
            }
            PlayerCommand::Pause => self.pause()?,
            PlayerCommand::Resume => self.resume()?,
            PlayerCommand::Stop => self.stop()?,
            PlayerCommand::Seek(position) => {
                let position = position.min(self.state.duration_ms);
                if self.loaded {
                    self.audio.send(AudioCommand::Seek(position))?;
                } else if self.state.current.is_some() {
                    self.begin(position, self.want_play, false)?;
                }
                self.state.position_ms = position;
            }
            PlayerCommand::Volume(value) => {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(Error::Invalid("Invalid volume".into()));
                }
                self.audio.send(AudioCommand::Volume(value))?;
                self.state.volume = value;
                let mut settings = self.storage.settings()?;
                settings.volume = value;
                self.storage.save_settings(&settings)?;
            }
            PlayerCommand::Next => {
                self.failed = 0;
                if self.queue.advance(false).is_some() {
                    self.begin(0, true, true)?;
                    queue_changed = true;
                } else {
                    self.stop()?;
                }
            }
            PlayerCommand::Previous => {
                self.failed = 0;
                if self.state.position_ms > 3000 {
                    self.audio.send(AudioCommand::Seek(0))?;
                    self.state.position_ms = 0;
                } else if self.queue.previous().is_some() {
                    self.begin(0, true, true)?;
                    queue_changed = true;
                }
            }
            PlayerCommand::Shuffle(value) => {
                self.queue.set_shuffle(value);
                self.state.shuffle = value;
                queue_changed = true;
            }
            PlayerCommand::Repeat(value) => {
                self.queue.repeat = value;
                self.state.repeat = value;
                queue_changed = true;
            }
            PlayerCommand::Device(device) => {
                self.audio.send(AudioCommand::Device(device))?;
            }
            PlayerCommand::Shutdown => {
                self.pause()?;
                self.save()?;
                self.audio.send(AudioCommand::Shutdown)?;
            }
        }
        if queue_changed {
            self.queue_tx.send_replace(self.queue.clone());
        }
        self.publish();
        self.save()?;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.want_play = false;
        self.audio.send(AudioCommand::Pause(true))?;
        if self.state.current.is_some() {
            self.state.status = PlaybackStatus::Paused;
        }
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        if self.queue.current.is_none() {
            self.queue.advance(false);
            self.queue_tx.send_replace(self.queue.clone());
        }
        if self.queue.current.is_none() {
            return Ok(());
        }
        self.want_play = true;
        if self.loaded {
            self.audio.send(AudioCommand::Pause(false))?;
            self.state.status = PlaybackStatus::Playing;
        } else {
            self.failed = 0;
            self.begin(self.state.position_ms, true, false)?;
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.generation += 1;
        if let Some(resolver) = self.resolver.take() {
            resolver.abort();
        }
        self.loaded = false;
        self.want_play = false;
        self.state.status = PlaybackStatus::Stopped;
        self.state.position_ms = 0;
        self.audio.send(AudioCommand::Stop)
    }
    fn begin(&mut self, position: u64, play: bool, reset_retry: bool) -> Result<()> {
        self.generation += 1;
        if let Some(resolver) = self.resolver.take() {
            resolver.abort();
        }
        self.audio.send(AudioCommand::Stop)?;
        self.loaded = false;
        self.want_play = play;
        if reset_retry {
            self.retries = 0;
            self.history_recorded = false;
        }
        let track = self
            .queue
            .current()
            .ok_or_else(|| Error::Invalid("Queue is empty".into()))?
            .track
            .clone();
        self.state.entry_id = self.queue.current;
        self.state.duration_ms = track.duration_ms;
        self.state.position_ms = position.min(track.duration_ms);
        self.state.current = Some(track.clone());
        self.state.status = PlaybackStatus::Loading;
        self.state.error = None;
        let providers = self.providers.clone();
        let sender = self.sender.clone();
        let generation = self.generation;
        self.resolver = Some(tokio::spawn(async move {
            let result = async {
                let source = track
                    .sources
                    .iter()
                    .find(|s| s.availability != Availability::Unavailable)
                    .ok_or(Error::Unavailable)?;
                providers
                    .get(&source.provider)?
                    .resolve_stream(source)
                    .await
            }
            .await;
            let _ = sender.send(Message::Resolved { generation, result }).await;
        }));
        Ok(())
    }
    fn resolved(&mut self, result: Result<StreamSource>) {
        match result {
            Ok(source) => {
                self.state.duration_ms = source.duration_ms;
                if let Err(e) = self.audio.send(AudioCommand::Load {
                    source,
                    position_ms: self.state.position_ms,
                    paused: !self.want_play,
                }) {
                    self.failure(e);
                }
            }
            Err(e) => self.failure(e),
        }
        self.publish();
    }
    fn event(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::Loaded if self.state.status == PlaybackStatus::Loading => {
                self.loaded = true;
                self.failed = 0;
                self.state.status = if self.want_play {
                    PlaybackStatus::Playing
                } else {
                    PlaybackStatus::Paused
                };
                if self.want_play && !self.history_recorded {
                    if let Some(track) = &self.state.current {
                        if let Err(e) = self.storage.record_play(track.internal_id) {
                            tracing::error!(error=%e,"history persistence failed");
                        } else {
                            self.history_recorded = true;
                        }
                    }
                }
            }
            AudioEvent::Position(position) if self.loaded => {
                self.state.position_ms = position.min(self.state.duration_ms.max(position));
                if self.last_save.elapsed() >= Duration::from_secs(10) {
                    if let Err(e) = self.save() {
                        tracing::error!(error=%e,"playback checkpoint failed");
                    }
                }
            }
            AudioEvent::Duration(duration) if self.loaded && duration > 0 => {
                self.state.duration_ms = duration
            }
            AudioEvent::Paused(paused) if self.loaded => {
                self.state.status = if paused {
                    PlaybackStatus::Paused
                } else {
                    PlaybackStatus::Playing
                };
            }
            AudioEvent::Buffering(value) if self.loaded && self.want_play => {
                self.state.status = if value {
                    PlaybackStatus::Buffering
                } else {
                    PlaybackStatus::Playing
                }
            }
            AudioEvent::Ended if self.loaded => {
                self.loaded = false;
                if self.queue.advance(true).is_some() {
                    if let Err(e) = self.begin(0, true, true) {
                        self.failure(e);
                    }
                    self.queue_tx.send_replace(self.queue.clone());
                } else {
                    self.want_play = false;
                    self.state.status = PlaybackStatus::Stopped;
                    self.state.position_ms = 0;
                }
                if let Err(e) = self.save() {
                    tracing::error!(error=%e,"queue persistence failed");
                }
            }
            AudioEvent::Error(message)
                if matches!(
                    self.state.status,
                    PlaybackStatus::Loading | PlaybackStatus::Playing | PlaybackStatus::Buffering
                ) =>
            {
                if self.retries < 1 {
                    self.retries += 1;
                    if let Err(e) = self.begin(self.state.position_ms, self.want_play, false) {
                        self.failure(e);
                    }
                } else {
                    self.failure(Error::Audio(message));
                }
            }
            _ => return,
        }
        self.publish();
    }
    fn failure(&mut self, error: Error) {
        tracing::warn!(error=%error,"playback failed");
        self.loaded = false;
        let unavailable = matches!(error, Error::Unavailable);
        self.state.error = Some(error.to_string());
        self.state.status = PlaybackStatus::Error;
        // Only unavailability is auto-skipped; outages stop instead of hammering the provider.
        self.failed += 1;
        if unavailable && self.want_play && self.failed < self.queue.entries.len().min(10) {
            if self.queue.advance(false).is_some() {
                if let Err(e) = self.begin(0, true, true) {
                    self.state.error = Some(e.to_string());
                    self.state.status = PlaybackStatus::Error;
                }
                self.queue_tx.send_replace(self.queue.clone());
            }
        } else {
            self.want_play = false;
        }
    }
    fn publish(&self) {
        self.state_tx.send_replace(self.state.clone());
    }
    fn save(&mut self) -> Result<()> {
        self.storage
            .save_queue(&self.queue, self.state.position_ms)?;
        self.last_save = Instant::now();
        Ok(())
    }
}
