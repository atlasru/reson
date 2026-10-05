use async_trait::async_trait;
use reson_core::{
    audio::{AudioBackend, AudioCommand, AudioEvent},
    error::{Error, Result},
    models::*,
    player::{PlaybackStatus, Player, PlayerCommand},
    providers::{Capability, MusicProvider, ProviderInfo, ProviderRegistry},
    storage::Storage,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
struct Provider {
    calls: Arc<AtomicUsize>,
}
#[async_trait]
impl MusicProvider for Provider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "test".into(),
            display_name: "Test".into(),
            mode: "test".into(),
            capabilities: vec![Capability::Playback],
        }
    }
    async fn search(&self, _: &str, _: u32, _: CancellationToken) -> Result<SearchResults> {
        Err(Error::Unsupported("search"))
    }
    async fn track(&self, _: &str) -> Result<Track> {
        Err(Error::Unsupported("track"))
    }
    async fn resolve_stream(&self, s: &TrackSource) -> Result<StreamSource> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if s.provider_id == "unavailable" {
            return Err(Error::Unavailable);
        }
        if s.provider_id == "network" {
            return Err(Error::Network);
        }
        Ok(StreamSource {
            url: "https://example.test/refreshed".into(),
            kind: StreamKind::Hls,
            duration_ms: 60000,
        })
    }
}
struct Audio {
    events: mpsc::UnboundedSender<AudioEvent>,
    loads: Arc<AtomicUsize>,
    volume: Mutex<f64>,
    auto_load: bool,
}
impl AudioBackend for Audio {
    fn send(&self, c: AudioCommand) -> Result<()> {
        match c {
            AudioCommand::Load { position_ms, .. } => {
                self.loads.fetch_add(1, Ordering::Relaxed);
                if self.auto_load {
                    let _ = self.events.send(AudioEvent::Loaded);
                    let _ = self.events.send(AudioEvent::Position(position_ms));
                }
            }
            AudioCommand::Pause(p) => {
                let _ = self.events.send(AudioEvent::Paused(p));
            }
            AudioCommand::Volume(v) => *self.volume.lock().unwrap() = v,
            _ => {}
        }
        Ok(())
    }
}
fn track(id: &str) -> Track {
    Track {
        internal_id: Uuid::new_v4(),
        title: id.into(),
        artists: vec![],
        album: None,
        artwork: None,
        duration_ms: 60000,
        explicit: None,
        availability: Availability::Playable,
        sources: vec![TrackSource {
            provider: "test".into(),
            provider_id: id.into(),
            availability: Availability::Playable,
            url: None,
        }],
    }
}
async fn setup() -> (
    tempfile::TempDir,
    Player,
    mpsc::UnboundedSender<AudioEvent>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    setup_with_loading(true).await
}
async fn setup_with_loading(
    auto_load: bool,
) -> (
    tempfile::TempDir,
    Player,
    mpsc::UnboundedSender<AudioEvent>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let loads = Arc::new(AtomicUsize::new(0));
    let mut registry = ProviderRegistry::default();
    registry.register(Arc::new(Provider {
        calls: calls.clone(),
    }));
    let (tx, rx) = mpsc::unbounded_channel();
    let audio = Arc::new(Audio {
        events: tx.clone(),
        loads: loads.clone(),
        volume: Mutex::new(1.),
        auto_load,
    });
    let player = Player::start(storage, registry, audio, rx).unwrap();
    (temp, player, tx, calls, loads)
}

#[tokio::test]
async fn pausing_during_load_accepts_late_native_ready_event() {
    let (temp, player, events, _, loads) = setup_with_loading(false).await;
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let tracks = storage.intern_tracks(vec![track("late-ready")]).unwrap();
    player
        .command(PlayerCommand::Play { tracks, index: 0 })
        .await
        .unwrap();
    let mut states = player.state.clone();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while loads.load(Ordering::Relaxed) == 0 {
            states.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    events.send(AudioEvent::Loaded).unwrap();
    wait(&player, PlaybackStatus::Paused).await;
    player.command(PlayerCommand::Resume).await.unwrap();
    wait(&player, PlaybackStatus::Playing).await;
    assert_eq!(
        loads.load(Ordering::Relaxed),
        1,
        "Resume must reuse the native stream loaded while paused"
    );
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
async fn wait(player: &Player, status: PlaybackStatus) {
    let mut state = player.state.clone();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if state.borrow().status == status {
                return;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn native_failure_refreshes_source_once_then_recovers() {
    let (_temp, player, events, calls, loads) = setup().await;
    let s = Storage::open(&_temp.path().join("state.sqlite")).unwrap();
    let tracks = s.intern_tracks(vec![track("playable")]).unwrap();
    player
        .command(PlayerCommand::Play { tracks, index: 0 })
        .await
        .unwrap();
    wait(&player, PlaybackStatus::Playing).await;
    events
        .send(AudioEvent::Error("Expired source".into()))
        .unwrap();
    let mut state = player.state.clone();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while loads.load(Ordering::Relaxed) < 2 {
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    wait(&player, PlaybackStatus::Playing).await;
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn unavailable_track_skips_without_poisoning_queue() {
    let (temp, player, _events, calls, _loads) = setup().await;
    let s = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let tracks = s
        .intern_tracks(vec![track("unavailable"), track("playable")])
        .unwrap();
    player
        .command(PlayerCommand::Play { tracks, index: 0 })
        .await
        .unwrap();
    wait(&player, PlaybackStatus::Playing).await;
    assert_eq!(
        player.state.borrow().current.as_ref().unwrap().title,
        "playable"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn network_failure_stops_without_retrying_every_queue_entry() {
    let (temp, player, _events, calls, _) = setup().await;
    let s = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let tracks = s
        .intern_tracks(vec![track("network"), track("playable")])
        .unwrap();
    player
        .command(PlayerCommand::Play { tracks, index: 0 })
        .await
        .unwrap();
    wait(&player, PlaybackStatus::Error).await;
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(player.queue.borrow().entries.len(), 2);
    player.command(PlayerCommand::Shutdown).await.unwrap();
}
#[tokio::test]
async fn seek_pause_volume_and_restart_state_are_core_owned() {
    let (temp, player, _events, _calls, _) = setup().await;
    let s = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let tracks = s
        .intern_tracks(vec![track("first"), track("second")])
        .unwrap();
    player
        .command(PlayerCommand::Play { tracks, index: 0 })
        .await
        .unwrap();
    wait(&player, PlaybackStatus::Playing).await;
    player.command(PlayerCommand::Seek(12000)).await.unwrap();
    player.command(PlayerCommand::Volume(0.3)).await.unwrap();
    player.command(PlayerCommand::Pause).await.unwrap();
    wait(&player, PlaybackStatus::Paused).await;
    assert_eq!(player.state.borrow().position_ms, 12000);
    assert_eq!(s.settings().unwrap().volume, 0.3);
    player.command(PlayerCommand::Next).await.unwrap();
    wait(&player, PlaybackStatus::Playing).await;
    assert_eq!(
        player.state.borrow().current.as_ref().unwrap().title,
        "second"
    );
    player.command(PlayerCommand::Shutdown).await.unwrap();
    let (q, _) = s.load_queue().unwrap();
    assert_eq!(q.current().unwrap().track.title, "second");
}
