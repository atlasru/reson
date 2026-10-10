#[cfg(target_os = "windows")]
pub fn initialize(window: &tauri::WebviewWindow, player: &reson_core::player::Player) {
    use reson_core::player::{PlaybackStatus, PlayerCommand};
    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
        PlatformConfig, SeekDirection,
    };
    use std::time::Duration;
    let Ok(hwnd) = window.hwnd() else {
        tracing::warn!("Windows media integration: no HWND");
        return;
    };
    let hwnd = hwnd.0 as usize;
    let callback_player = player.clone();
    let mut snapshots = player.state.clone();
    let (sender, receiver) = std::sync::mpsc::channel::<reson_core::player::PlayerState>();
    let worker = std::thread::Builder::new()
        .name("reson-windows-media".into())
        .spawn(move || {
            let config = PlatformConfig {
                dbus_name: "reson",
                display_name: "Reson",
                hwnd: Some(hwnd as *mut std::ffi::c_void),
            };
            let _runtime = match WinRtRuntime::initialize() {
                Ok(runtime) => runtime,
                Err(code) => {
                    tracing::warn!(code, "Windows Runtime initialization failed");
                    return;
                }
            };
            let mut controls = match MediaControls::new(config) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error=%e,"Windows media session unavailable");
                    return;
                }
            };
            if let Err(e) = controls.attach(move |event| {
                let snapshot = callback_player.state.borrow().clone();
                let command = match event {
                    MediaControlEvent::Play => PlayerCommand::Resume,
                    MediaControlEvent::Pause => PlayerCommand::Pause,
                    MediaControlEvent::Toggle => PlayerCommand::Toggle,
                    MediaControlEvent::Next => PlayerCommand::Next,
                    MediaControlEvent::Previous => PlayerCommand::Previous,
                    MediaControlEvent::Stop => PlayerCommand::Stop,
                    MediaControlEvent::SetPosition(position) => {
                        PlayerCommand::Seek(position.0.as_millis() as u64)
                    }
                    MediaControlEvent::Seek(direction) => PlayerCommand::Seek(match direction {
                        SeekDirection::Forward => snapshot.position_ms.saturating_add(10000),
                        SeekDirection::Backward => snapshot.position_ms.saturating_sub(10000),
                    }),
                    MediaControlEvent::SeekBy(direction, amount) => {
                        PlayerCommand::Seek(match direction {
                            SeekDirection::Forward => snapshot
                                .position_ms
                                .saturating_add(amount.as_millis() as u64),
                            SeekDirection::Backward => snapshot
                                .position_ms
                                .saturating_sub(amount.as_millis() as u64),
                        })
                    }
                    MediaControlEvent::SetVolume(value) => {
                        PlayerCommand::Volume(value.clamp(0.0, 1.0))
                    }
                    _ => return,
                };
                let player = callback_player.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = player.command(command).await {
                        tracing::warn!(error=%e,"Windows media command failed");
                    }
                });
            }) {
                tracing::warn!(error=%e,"Windows media controls attach failed");
                return;
            }
            let mut previous = None;
            while let Ok(state) = receiver.recv() {
                if Some((state.entry_id, state.duration_ms)) != previous {
                    previous = Some((state.entry_id, state.duration_ms));
                    if let Some(track) = &state.current {
                        let artist = track.artist_name();
                        let metadata = MediaMetadata {
                            title: Some(&track.title),
                            artist: Some(&artist),
                            album: track.album.as_deref(),
                            cover_url: track.artwork.as_deref(),
                            duration: Some(Duration::from_millis(state.duration_ms)),
                        };
                        if let Err(e) = controls.set_metadata(metadata) {
                            tracing::debug!(error=%e,"Windows media metadata update failed");
                        }
                    }
                }
                let progress = Some(MediaPosition(Duration::from_millis(state.position_ms)));
                let playback = match state.status {
                    PlaybackStatus::Playing | PlaybackStatus::Buffering => {
                        MediaPlayback::Playing { progress }
                    }
                    PlaybackStatus::Paused | PlaybackStatus::Loading => {
                        MediaPlayback::Paused { progress }
                    }
                    _ => MediaPlayback::Stopped,
                };
                let _ = controls.set_playback(playback);
            }
            let _ = controls.detach();
        });
    if let Err(error) = worker {
        tracing::warn!(%error, "Windows media worker unavailable");
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut last = String::new();
        loop {
            let state = snapshots.borrow().clone();
            let key = format!(
                "{:?}:{:?}:{}",
                state.entry_id,
                state.status,
                state.position_ms / 2000
            );
            if key != last {
                last = key;
                if sender.send(state).is_err() {
                    break;
                }
            }
            if snapshots.changed().await.is_err() {
                break;
            }
        }
    });
}
#[cfg(not(target_os = "windows"))]
pub fn initialize(_window: &tauri::WebviewWindow, _player: &reson_core::player::Player) {}

#[cfg(target_os = "windows")]
struct WinRtRuntime;
#[cfg(target_os = "windows")]
impl WinRtRuntime {
    fn initialize() -> std::result::Result<Self, i32> {
        let status = unsafe { RoInitialize(1) };
        if status < 0 {
            Err(status)
        } else {
            Ok(Self)
        }
    }
}
#[cfg(target_os = "windows")]
impl Drop for WinRtRuntime {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}
#[cfg(target_os = "windows")]
#[link(name = "runtimeobject")]
extern "system" {
    fn RoInitialize(kind: u32) -> i32;
    fn RoUninitialize();
}
