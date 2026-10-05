use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use reson_core::{
    config::Settings,
    player::{PlaybackStatus, Player},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
static GENERATION: AtomicU64 = AtomicU64::new(0);
static SENDER: OnceLock<std::sync::mpsc::Sender<Message>> = OnceLock::new();
enum Message {
    Configure(u64, Settings),
    State(u64, reson_core::player::PlayerState),
}
pub fn configure(settings: Settings, player: &Player) {
    let sender = SENDER.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("reson-discord".into())
            .spawn(move || worker(receiver))
            .expect("Discord worker");
        sender
    });
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = sender.send(Message::Configure(generation, settings));
    let mut state = player.state.clone();
    let sender = sender.clone();
    tauri::async_runtime::spawn(async move {
        let mut last = String::new();
        while state.changed().await.is_ok() {
            if GENERATION.load(Ordering::Relaxed) != generation {
                break;
            }
            let snapshot = state.borrow().clone();
            let key = format!(
                "{:?}-{:?}-{}",
                snapshot.entry_id,
                snapshot.status,
                snapshot.position_ms / 15000
            );
            if key == last {
                continue;
            }
            last = key;
            let _ = sender.send(Message::State(generation, snapshot));
        }
    });
}
fn worker(receiver: std::sync::mpsc::Receiver<Message>) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut generation = 0;
    let mut enabled = false;
    let mut application_id = String::new();
    let mut last_attempt = std::time::Instant::now() - std::time::Duration::from_secs(30);
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Configure(g, settings) => {
                generation = g;
                enabled = settings.discord_presence;
                application_id = settings.discord_application_id;
                if let Some(mut old) = client.take() {
                    let _ = old.clear_activity();
                    let _ = old.close();
                }
            }
            Message::State(g, state) if g == generation && enabled => {
                if client.is_none() && last_attempt.elapsed() > std::time::Duration::from_secs(20) {
                    last_attempt = std::time::Instant::now();
                    if let Ok(mut connection) = DiscordIpcClient::new(&application_id) {
                        if connection.connect().is_ok() {
                            client = Some(connection);
                        }
                    }
                }
                if let Some(connection) = &mut client {
                    let result = if state.status == PlaybackStatus::Playing {
                        if let Some(track) = &state.current {
                            let details = truncate(&track.title);
                            let artist = truncate(&track.artist_name());
                            let now = reson_core::storage::now();
                            let start = now - state.position_ms as i64 / 1000;
                            let end = start + state.duration_ms as i64 / 1000;
                            let assets = if let Some(artwork) = &track.artwork {
                                activity::Assets::new()
                                    .large_image(artwork)
                                    .large_text("Reson")
                            } else {
                                activity::Assets::new()
                            };
                            connection.set_activity(
                                activity::Activity::new()
                                    .activity_type(activity::ActivityType::Listening)
                                    .details(&details)
                                    .state(&artist)
                                    .timestamps(activity::Timestamps::new().start(start).end(end))
                                    .assets(assets),
                            )
                        } else {
                            connection.clear_activity()
                        }
                    } else {
                        connection.clear_activity()
                    };
                    if result.is_err() {
                        client = None;
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(mut client) = client {
        let _ = client.clear_activity();
        let _ = client.close();
    }
}
fn truncate(s: &str) -> String {
    let s = s.chars().take(120).collect::<String>();
    if s.chars().count() < 2 {
        format!("{s} ")
    } else {
        s
    }
}
