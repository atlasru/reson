use reson_core::{
    audio::{AudioBackend, AudioDevice},
    cache::ArtworkCache,
    config::Settings,
    error::{Error, Result},
    library::Library,
    models::*,
    player::{Player, PlayerCommand, PlayerState},
    providers::{ProviderInfo, ProviderRegistry},
    queue::{Queue, RepeatMode},
    storage::{Installation, Storage},
};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct AppState {
    pub storage: Storage,
    pub providers: ProviderRegistry,
    pub player: Player,
    pub audio: Arc<dyn AudioBackend>,
    pub cache: ArtworkCache,
    pub requests: Mutex<HashMap<String, CancellationToken>>,
    pub audio_error: Option<String>,
    pub logs: PathBuf,
    pub data_dir: PathBuf,
}
#[derive(Serialize)]
pub struct Snapshot {
    player: PlayerState,
    queue: Queue,
    library: Library,
    settings: Settings,
    providers: Vec<ProviderInfo>,
    installation: Installation,
    audio_error: Option<String>,
}
#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Result<Snapshot> {
    Ok(Snapshot {
        player: state.player.state.borrow().clone(),
        queue: state.player.queue.borrow().clone(),
        library: Library::load(&state.storage)?,
        settings: state.storage.settings()?,
        providers: state.providers.list(),
        installation: state.storage.installation()?,
        audio_error: state.audio_error.clone(),
    })
}
#[tauri::command]
pub async fn search(
    state: State<'_, AppState>,
    provider: String,
    query: String,
    offset: u32,
    request_id: String,
) -> Result<SearchResults> {
    if request_id.len() > 80 {
        return Err(Error::Invalid("Invalid request identifier".into()));
    }
    let cancel = CancellationToken::new();
    {
        let mut requests = state.requests.lock().await;
        if requests.len() >= 16 {
            return Err(Error::Invalid("Too many searches in progress".into()));
        }
        if let Some(old) = requests.insert(request_id.clone(), cancel.clone()) {
            old.cancel();
        }
    }
    let result = state
        .providers
        .get(&provider)?
        .search(&query, offset, cancel)
        .await;
    state.requests.lock().await.remove(&request_id);
    let mut result = result?;
    result.tracks = state.storage.intern_tracks(result.tracks)?;
    result.artists = state.storage.intern_artists(result.artists)?;
    for p in &mut result.playlists {
        normalize_playlist(&state.storage, p)?;
    }
    Ok(result)
}
#[tauri::command]
pub async fn cancel_request(state: State<'_, AppState>, request_id: String) -> Result<()> {
    if let Some(cancel) = state.requests.lock().await.remove(&request_id) {
        cancel.cancel();
    }
    Ok(())
}
#[tauri::command]
pub async fn discover(state: State<'_, AppState>, provider: String) -> Result<Vec<Track>> {
    state
        .storage
        .intern_tracks(state.providers.get(&provider)?.discover().await?)
}
#[tauri::command]
pub async fn related_tracks(
    state: State<'_, AppState>,
    provider: String,
    id: String,
) -> Result<Vec<Track>> {
    state
        .storage
        .intern_tracks(state.providers.get(&provider)?.related(&id).await?)
}
#[tauri::command]
pub async fn artist_page(
    state: State<'_, AppState>,
    provider: String,
    id: String,
    offset: u32,
) -> Result<ArtistPage> {
    let mut page = state.providers.get(&provider)?.artist(&id, offset).await?;
    page.tracks = state.storage.intern_tracks(page.tracks)?;
    page.artist = state.storage.intern_artists(vec![page.artist])?.remove(0);
    for p in &mut page.playlists {
        normalize_playlist(&state.storage, p)?;
    }
    Ok(page)
}
#[tauri::command]
pub async fn provider_playlist(
    state: State<'_, AppState>,
    provider: String,
    id: String,
) -> Result<Playlist> {
    let mut p = state.providers.get(&provider)?.playlist(&id).await?;
    normalize_playlist(&state.storage, &mut p)?;
    Ok(p)
}
#[tauri::command]
pub async fn resolve_url(
    state: State<'_, AppState>,
    provider: String,
    url: String,
) -> Result<ResolvedEntity> {
    let mut entity = state.providers.get(&provider)?.resolve_url(&url).await?;
    match &mut entity {
        ResolvedEntity::Track(t) => *t = state.storage.intern_tracks(vec![t.clone()])?.remove(0),
        ResolvedEntity::Artist(a) => *a = state.storage.intern_artists(vec![a.clone()])?.remove(0),
        ResolvedEntity::Playlist(p) => normalize_playlist(&state.storage, p)?,
    };
    Ok(entity)
}
fn normalize_playlist(storage: &Storage, p: &mut Playlist) -> Result<()> {
    p.tracks = storage.intern_tracks(std::mem::take(&mut p.tracks))?;
    if let Some(owner) = p.owner.take() {
        p.owner = Some(storage.intern_artists(vec![owner])?.remove(0));
    }
    Ok(())
}
fn load_tracks(storage: &Storage, ids: Vec<Uuid>) -> Result<Vec<Track>> {
    if ids.len() > Queue::LIMIT {
        return Err(Error::Invalid("Too many tracks".into()));
    }
    ids.into_iter().map(|id| storage.track(id)).collect()
}
#[tauri::command]
pub async fn play_tracks(state: State<'_, AppState>, ids: Vec<Uuid>, index: usize) -> Result<()> {
    state
        .player
        .command(PlayerCommand::Play {
            tracks: load_tracks(&state.storage, ids)?,
            index,
        })
        .await
}
#[tauri::command]
pub async fn enqueue_tracks(state: State<'_, AppState>, ids: Vec<Uuid>, next: bool) -> Result<()> {
    state
        .player
        .command(PlayerCommand::Enqueue {
            tracks: load_tracks(&state.storage, ids)?,
            next,
        })
        .await
}
#[tauri::command]
pub async fn player_control(
    state: State<'_, AppState>,
    action: String,
    value: serde_json::Value,
) -> Result<()> {
    let bad = || Error::Invalid("Invalid playback control value".into());
    let command = match action.as_str() {
        "toggle" => PlayerCommand::Toggle,
        "pause" => PlayerCommand::Pause,
        "resume" => PlayerCommand::Resume,
        "stop" => PlayerCommand::Stop,
        "next" => PlayerCommand::Next,
        "previous" => PlayerCommand::Previous,
        "seek" => PlayerCommand::Seek(value.as_u64().ok_or_else(bad)?),
        "volume" => PlayerCommand::Volume(value.as_f64().ok_or_else(bad)?),
        "shuffle" => PlayerCommand::Shuffle(value.as_bool().ok_or_else(bad)?),
        "repeat" => PlayerCommand::Repeat(match value.as_str() {
            Some("off") => RepeatMode::Off,
            Some("queue") => RepeatMode::Queue,
            Some("track") => RepeatMode::Track,
            _ => return Err(bad()),
        }),
        _ => return Err(Error::Invalid("Unknown playback control".into())),
    };
    state.player.command(command).await
}
#[tauri::command]
pub async fn queue_select(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    state.player.command(PlayerCommand::Select(id)).await
}
#[tauri::command]
pub async fn queue_remove(state: State<'_, AppState>, id: Uuid) -> Result<()> {
    state.player.command(PlayerCommand::Remove(id)).await
}
#[tauri::command]
pub async fn queue_reorder(state: State<'_, AppState>, entry_id: Uuid, to: usize) -> Result<()> {
    state
        .player
        .command(PlayerCommand::Reorder { entry_id, to })
        .await
}
#[tauri::command]
pub async fn queue_clear(state: State<'_, AppState>) -> Result<()> {
    state.player.command(PlayerCommand::Clear).await
}
#[tauri::command]
pub fn library_state(state: State<'_, AppState>) -> Result<Library> {
    Library::load(&state.storage)
}
fn publish_library(app: &tauri::AppHandle, state: &AppState) -> Result<()> {
    app.emit("library-state", Library::load(&state.storage)?)
        .map_err(|_| Error::Invalid("Cannot synchronize library".into()))
}
#[tauri::command]
pub fn set_favorite(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    enabled: bool,
) -> Result<()> {
    state.storage.favorite(id, enabled)?;
    publish_library(&app, &state)
}
#[tauri::command]
pub fn create_playlist(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    title: String,
) -> Result<Uuid> {
    let id = state.storage.create_playlist(&title)?;
    publish_library(&app, &state)?;
    Ok(id)
}
#[tauri::command]
pub fn rename_playlist(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    title: String,
) -> Result<()> {
    state.storage.rename_playlist(id, &title)?;
    publish_library(&app, &state)
}
#[tauri::command]
pub fn delete_playlist(app: tauri::AppHandle, state: State<'_, AppState>, id: Uuid) -> Result<()> {
    state.storage.delete_playlist(id)?;
    publish_library(&app, &state)
}
#[tauri::command]
pub fn add_to_playlist(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    ids: Vec<Uuid>,
) -> Result<()> {
    state.storage.add_to_playlist(id, &ids)?;
    publish_library(&app, &state)
}
#[tauri::command]
pub fn remove_from_playlist(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    position: usize,
) -> Result<()> {
    state.storage.remove_from_playlist(id, position)?;
    publish_library(&app, &state)
}
#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<()> {
    settings.validate()?;
    if settings.telemetry {
        return Err(Error::Unsupported("telemetry in this build"));
    }
    let old = state.storage.settings()?;
    state
        .player
        .command(PlayerCommand::Device(settings.audio_device.clone()))
        .await?;
    state.cache.set_limit(settings.cache_limit_mb).await?;
    state.storage.save_settings(&settings)?;
    if (old.volume - settings.volume).abs() > 0.001 {
        state
            .player
            .command(PlayerCommand::Volume(settings.volume))
            .await?;
    }
    app.emit("settings-state", &settings)
        .map_err(|_| Error::Invalid("Cannot synchronize settings".into()))?;
    crate::discord::configure(settings, &state.player);
    Ok(())
}
#[tauri::command]
pub async fn cached_artwork(state: State<'_, AppState>, url: String) -> Result<String> {
    Ok(state.cache.get(&url).await?.to_string_lossy().into_owned())
}
#[tauri::command]
pub fn cache_size(state: State<'_, AppState>) -> u64 {
    state.cache.size()
}
#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<()> {
    state.cache.clear().await
}
#[tauri::command]
pub async fn audio_devices(state: State<'_, AppState>) -> Result<Vec<AudioDevice>> {
    let audio = state.audio.clone();
    tokio::task::spawn_blocking(move || audio.devices())
        .await
        .map_err(|_| Error::Audio("Device enumeration interrupted".into()))?
}
#[tauri::command]
pub fn open_external(url: String) -> Result<()> {
    let u = url::Url::parse(&url).map_err(|_| Error::Invalid("Invalid external link".into()))?;
    if u.scheme() != "https"
        || !matches!(
            u.host_str(),
            Some("soundcloud.com" | "www.soundcloud.com" | "github.com")
        )
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some_and(|p| p != 443)
    {
        return Err(Error::Invalid("Unsupported external link".into()));
    }
    open::that_detached(url).map_err(|_| Error::Invalid("Cannot open the system browser".into()))
}
#[tauri::command]
pub async fn export_diagnostics(state: State<'_, AppState>) -> Result<String> {
    crate::diagnostics::export(&state.logs, &state.data_dir, state.audio_error.clone()).await
}
