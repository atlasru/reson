#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod diagnostics;
mod discord;
mod smoke;
mod windows;

use commands::AppState;
use reson_core::{
    audio::{AudioBackend, AudioCommand, NativeAudio},
    cache::ArtworkCache,
    error::{Error, Result},
    player::{Player, PlayerCommand},
    providers::{soundcloud::SoundCloudProvider, ProviderRegistry},
    storage::Storage,
};
use std::sync::Arc;
use tauri::{Emitter, Manager};

struct UnavailableAudio(String);
impl AudioBackend for UnavailableAudio {
    fn send(&self, command: AudioCommand) -> Result<()> {
        if matches!(command, AudioCommand::Load { .. } | AudioCommand::Seek(_)) {
            Err(Error::Audio(self.0.clone()))
        } else {
            Ok(())
        }
    }
}
fn main() {
    if std::env::args().any(|a| a == "--smoke") {
        let result = tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(smoke::run());
        if let Err(e) = result {
            eprintln!("Smoke validation failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    let application = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let logs = app.path().app_log_dir()?;
            app.manage(std::sync::Mutex::new(diagnostics::initialize(&logs)?));
            let storage = Storage::open(&data_dir.join("reson.sqlite"))?;
            storage.recover_imports()?;
            let settings = storage.settings()?;
            let mut providers = ProviderRegistry::default();
            providers.register(Arc::new(SoundCloudProvider::new()?));
            let imports = reson_core::library::imports::ImportManager::new(
                storage.clone(),
                providers.clone(),
            );
            let mut import_events = imports.subscribe();
            let import_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match import_events.recv().await {
                        Ok(progress) => {
                            let _ = import_handle.emit("import-progress", progress);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            let native = app.path().resource_dir()?.join("libmpv-2.dll");
            let native_path = if cfg!(target_os = "windows") {
                Some(native)
            } else {
                None
            };
            let (audio, events, audio_error) = match NativeAudio::start(native_path, None) {
                Ok((audio, events)) => (audio, events, None),
                Err(e) => {
                    tracing::error!(error=%e,"native audio initialization failed");
                    let (_, events) = tokio::sync::mpsc::unbounded_channel();
                    let text = e.to_string();
                    (
                        Arc::new(UnavailableAudio(text.clone())) as Arc<dyn AudioBackend>,
                        events,
                        Some(text),
                    )
                }
            };
            let player = tauri::async_runtime::block_on(async {
                Player::start(storage.clone(), providers.clone(), audio.clone(), events)
            })?;
            let cache = ArtworkCache::new(
                app.path().app_cache_dir()?.join("artwork"),
                settings.cache_limit_mb,
                providers.artwork_hosts(),
            )?;
            let state = AppState {
                storage: storage.clone(),
                providers,
                player: player.clone(),
                audio,
                cache,
                requests: Default::default(),
                audio_error,
                logs,
                data_dir,
                frontend_connected: Default::default(),
                imports,
            };
            app.manage(state);
            let mut snapshots = player.state.clone();
            let handle = app.handle().clone();
            let history_storage = storage.clone();
            tauri::async_runtime::spawn(async move {
                let mut previous = None;
                while snapshots.changed().await.is_ok() {
                    let snapshot = snapshots.borrow().clone();
                    let _ = handle.emit("player-state", &snapshot);
                    if snapshot.entry_id != previous
                        && snapshot.status == reson_core::player::PlaybackStatus::Playing
                    {
                        previous = snapshot.entry_id;
                        if let Ok(library) = reson_core::library::Library::load(&history_storage) {
                            let _ = handle.emit("library-state", library);
                        }
                    }
                }
            });
            let mut queues = player.queue.clone();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while queues.changed().await.is_ok() {
                    let snapshot = queues.borrow().clone();
                    let _ = handle.emit("queue-state", snapshot);
                }
            });
            if let Some(window) = app.get_webview_window("main") {
                windows::initialize(&window, &player);
            }
            discord::configure(settings, &player);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "Reson started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::search,
            commands::cancel_request,
            commands::discover,
            commands::related_tracks,
            commands::artist_page,
            commands::provider_playlist,
            commands::resolve_url,
            commands::play_tracks,
            commands::enqueue_tracks,
            commands::player_control,
            commands::queue_select,
            commands::queue_remove,
            commands::queue_reorder,
            commands::queue_clear,
            commands::library_state,
            commands::start_likes_import,
            commands::cancel_likes_import,
            commands::import_state,
            commands::remove_import_source,
            commands::remove_saved_track,
            commands::set_favorite,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::update_settings,
            commands::cached_artwork,
            commands::cache_size,
            commands::clear_cache,
            commands::audio_devices,
            commands::open_external,
            commands::export_diagnostics
        ])
        .build(tauri::generate_context!());
    match application {
        Ok(application) => application.run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                    let player = app.state::<AppState>().player.clone();
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = player.command(PlayerCommand::Shutdown).await;
                        handle.exit(0);
                    });
                }
            }
        }),
        Err(e) => {
            eprintln!("Reson startup failed: {e}");
            std::process::exit(1);
        }
    }
}
