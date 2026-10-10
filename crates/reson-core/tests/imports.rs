use async_trait::async_trait;
use reson_core::{
    error::{Error, Result},
    library::{imports::*, Library},
    models::*,
    providers::{soundcloud::parse, Capability, MusicProvider, ProviderInfo, ProviderRegistry},
    storage::Storage,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
enum Response {
    Page(LikesPage),
    Network,
    Hidden,
    Pending,
    Repeat,
}
struct MockProvider {
    pages: Mutex<Vec<Response>>,
    resolving: AtomicBool,
    rate_once: AtomicBool,
    requests: AtomicUsize,
}
impl MockProvider {
    fn new(pages: Vec<Response>) -> Arc<Self> {
        Arc::new(Self {
            pages: Mutex::new(pages),
            resolving: AtomicBool::new(false),
            rate_once: AtomicBool::new(false),
            requests: AtomicUsize::new(0),
        })
    }
}
#[async_trait]
impl MusicProvider for MockProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "fixture".into(),
            display_name: "Fixture".into(),
            mode: "guest".into(),
            capabilities: vec![Capability::PublicLikesImport],
        }
    }
    fn normalize_profile_input(&self, input: &str) -> Result<String> {
        Ok(input.trim().to_owned())
    }
    async fn public_profile(
        &self,
        input: &str,
        _cancel: CancellationToken,
    ) -> Result<ImportProfile> {
        if self.resolving.load(Ordering::Relaxed) {
            std::future::pending::<()>().await;
        }
        Ok(ImportProfile {
            provider: "fixture".into(),
            provider_user_id: input.into(),
            url: format!("https://soundcloud.com/{input}"),
            name: input.into(),
            artwork: None,
        })
    }
    async fn liked_tracks_page(
        &self,
        _id: &str,
        cursor: Option<&str>,
        _cancel: CancellationToken,
    ) -> Result<LikesPage> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        if self.rate_once.swap(false, Ordering::Relaxed) {
            return Err(Error::RateLimited(1800));
        }
        let index = cursor.unwrap_or("0").parse::<usize>().unwrap();
        let response = self
            .pages
            .lock()
            .unwrap()
            .get(index)
            .cloned()
            .unwrap_or(Response::Network);
        match response {
            Response::Page(mut page) => {
                if index + 1 < self.pages.lock().unwrap().len() {
                    page.next = Some((index + 1).to_string());
                }
                Ok(page)
            }
            Response::Network => Err(Error::Network),
            Response::Hidden => Err(Error::Invalid("Likes are hidden".into())),
            Response::Pending => std::future::pending().await,
            Response::Repeat => Ok(LikesPage {
                next: Some("0".into()),
                ..Default::default()
            }),
        }
    }
}
fn tracks(count: usize, start: usize) -> Vec<Track> {
    (start..start+count).map(|id|parse::track(&serde_json::json!({"id":id+1,"title":format!("Track {id}"),"duration":180000,"streamable":true,"user":{"id":1,"username":"Creator"},"permalink_url":format!("https://soundcloud.com/creator/track-{id}")})).unwrap()).collect()
}
fn page(count: usize, start: usize) -> Response {
    Response::Page(LikesPage {
        tracks: tracks(count, start),
        discovered: count,
        ..Default::default()
    })
}
fn manager(storage: &Storage, provider: Arc<MockProvider>) -> ImportManager {
    let mut registry = ProviderRegistry::default();
    registry.register(provider);
    ImportManager::new(storage.clone(), registry)
}
async fn wait(
    manager: &ImportManager,
    id: Uuid,
    predicate: impl Fn(&ImportProgress) -> bool,
) -> ImportProgress {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(p) = manager.progress().into_iter().find(|p| p.job_id == id) {
                if predicate(&p) {
                    return p;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("import progress timeout")
}
async fn finished(manager: &ImportManager, id: Uuid) -> ImportProgress {
    wait(manager, id, |p| !p.status.active()).await
}

#[tokio::test]
async fn all_pages_idempotency_incremental_refresh_and_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.sqlite");
    let storage = Storage::open(&path).unwrap();
    let provider = MockProvider::new(vec![page(100, 0), page(100, 100), page(7, 200)]);
    let manager = manager(&storage, provider.clone());
    let p = manager.start("fixture", "first").unwrap();
    let done = finished(&manager, p.job_id).await;
    assert_eq!(done.status, ImportStatus::Complete);
    assert_eq!(done.pages, 3);
    assert_eq!(done.discovered, 207);
    assert_eq!(done.saved, 207);
    assert_eq!(done.duplicates, 0);
    let ids = storage
        .favorites()
        .unwrap()
        .iter()
        .map(|t| t.internal_id)
        .collect::<std::collections::HashSet<_>>();
    let p = manager.start("fixture", "first").unwrap();
    let done = finished(&manager, p.job_id).await;
    assert_eq!(done.saved, 0);
    assert_eq!(done.duplicates, 207);
    *provider.pages.lock().unwrap() = vec![page(1, 207), page(10, 0)];
    let done = finished(&manager, manager.start("fixture", "first").unwrap().job_id).await;
    assert_eq!(done.saved, 1);
    assert_eq!(storage.favorites().unwrap().len(), 208);
    assert_eq!(storage.import_sources().unwrap().len(), 1);
    drop(manager);
    drop(storage);
    let storage = Storage::open(&path).unwrap();
    assert_eq!(storage.favorites().unwrap().len(), 208);
    assert!(ids.iter().all(|id| storage.track(*id).is_ok()));
    assert!(storage.local_favorite_ids().unwrap().is_empty());
    assert_eq!(storage.import_track_sources().unwrap().len(), 208);
}
#[tokio::test]
async fn empty_and_hidden_are_not_confused() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let provider = MockProvider::new(vec![page(0, 0)]);
    let manager = manager(&storage, provider.clone());
    let done = finished(&manager, manager.start("fixture", "empty").unwrap().job_id).await;
    assert_eq!(done.status, ImportStatus::Complete);
    assert_eq!(done.discovered, 0);
    *provider.pages.lock().unwrap() = vec![Response::Hidden];
    let done = finished(&manager, manager.start("fixture", "hidden").unwrap().job_id).await;
    assert_eq!(done.status, ImportStatus::Failed);
    assert!(done.message.unwrap().contains("hidden"));
}
#[tokio::test]
async fn removing_a_source_cancels_refresh_even_during_profile_resolution() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let provider = MockProvider::new(vec![page(10, 0)]);
    let manager = manager(&storage, provider.clone());
    let initial = finished(
        &manager,
        manager.start("fixture", "creator").unwrap().job_id,
    )
    .await;
    provider.resolving.store(true, Ordering::Relaxed);
    let refresh = manager.start("fixture", "creator").unwrap();
    manager.cancel_source(initial.source_id.unwrap());
    storage
        .remove_import_source(initial.source_id.unwrap(), true)
        .unwrap();
    assert_eq!(
        finished(&manager, refresh.job_id).await.status,
        ImportStatus::Cancelled
    );
    assert!(storage.import_sources().unwrap().is_empty());
    assert!(storage.favorites().unwrap().is_empty());
    // A later explicit import remains allowed.
    provider.resolving.store(false, Ordering::Relaxed);
    let new = finished(
        &manager,
        manager.start("fixture", "creator").unwrap().job_id,
    )
    .await;
    assert_eq!(new.status, ImportStatus::Complete);
    assert_eq!(storage.favorites().unwrap().len(), 10);
}
#[tokio::test]
async fn cancellation_during_profile_resolution_request_and_cooldown_is_immediate() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let provider = MockProvider::new(vec![Response::Pending]);
    let manager = manager(&storage, provider.clone());
    provider.resolving.store(true, Ordering::Relaxed);
    let p = manager.start("fixture", "resolving").unwrap();
    manager.cancel(p.job_id);
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(500), finished(&manager, p.job_id))
            .await
            .unwrap()
            .status,
        ImportStatus::Cancelled
    );
    provider.resolving.store(false, Ordering::Relaxed);
    let p = manager.start("fixture", "request").unwrap();
    wait(&manager, p.job_id, |p| p.source_id.is_some()).await;
    manager.cancel(p.job_id);
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(500), finished(&manager, p.job_id))
            .await
            .unwrap()
            .status,
        ImportStatus::Cancelled
    );
    provider.rate_once.store(true, Ordering::Relaxed);
    let p = manager.start("fixture", "cooldown").unwrap();
    let waiting = wait(&manager, p.job_id, |p| p.status == ImportStatus::Cooldown).await;
    assert!(waiting.retry_at.is_some());
    manager.cancel(p.job_id);
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(500), finished(&manager, p.job_id))
            .await
            .unwrap()
            .status,
        ImportStatus::Cancelled
    );
}
#[tokio::test]
async fn partial_failure_and_cancellation_preserve_committed_pages() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let provider = MockProvider::new(vec![page(20, 0), Response::Network]);
    let manager = manager(&storage, provider.clone());
    let done = finished(&manager, manager.start("fixture", "first").unwrap().job_id).await;
    assert_eq!(done.status, ImportStatus::Partial);
    assert_eq!(done.saved, 20);
    assert_eq!(storage.favorites().unwrap().len(), 20);
    *provider.pages.lock().unwrap() = vec![page(20, 0), page(10, 20)];
    let done = finished(&manager, manager.start("fixture", "first").unwrap().job_id).await;
    assert_eq!(done.status, ImportStatus::Complete);
    assert_eq!(done.saved, 10);
    assert_eq!(done.duplicates, 20);
    *provider.pages.lock().unwrap() = vec![page(10, 30), Response::Pending];
    let p = manager.start("fixture", "second").unwrap();
    wait(&manager, p.job_id, |p| p.pages == 1).await;
    manager.cancel(p.job_id);
    let done = finished(&manager, p.job_id).await;
    assert_eq!(done.status, ImportStatus::Cancelled);
    assert_eq!(done.saved, 10);
    assert_eq!(storage.favorites().unwrap().len(), 40);
}
#[tokio::test]
async fn multiple_sources_concurrent_deduplication_source_removal_and_track_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let provider = MockProvider::new(vec![page(10, 0)]);
    let manager = manager(&storage, provider);
    let a = manager.start("fixture", "a").unwrap();
    let b = manager.start("fixture", "b").unwrap();
    let a = finished(&manager, a.job_id).await;
    let b = finished(&manager, b.job_id).await;
    assert_eq!(a.saved + b.saved, 10);
    assert_eq!(storage.favorites().unwrap().len(), 10);
    assert_eq!(
        storage
            .import_track_sources()
            .unwrap()
            .values()
            .filter(|s| s.len() == 2)
            .count(),
        10
    );
    let track = storage.favorites().unwrap()[0].internal_id;
    storage.favorite(track, true).unwrap();
    storage
        .remove_import_source(a.source_id.unwrap(), true)
        .unwrap();
    assert_eq!(storage.favorites().unwrap().len(), 10);
    storage
        .remove_saved_track(storage.favorites().unwrap()[1].internal_id)
        .unwrap();
    assert_eq!(storage.favorites().unwrap().len(), 9);
    finished(&manager, manager.start("fixture", "b").unwrap().job_id).await;
    assert_eq!(storage.favorites().unwrap().len(), 9);
    storage
        .remove_import_source(b.source_id.unwrap(), true)
        .unwrap();
    assert_eq!(storage.favorites().unwrap().len(), 1);
    assert_eq!(storage.favorites().unwrap()[0].internal_id, track);
}
#[tokio::test]
async fn repeat_cursor_reports_partial_and_unavailable_tracks_are_retained() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let mut items = tracks(1, 0);
    items[0].availability = Availability::Unavailable;
    items[0].sources[0].availability = Availability::Unavailable;
    let provider = MockProvider::new(vec![Response::Page(LikesPage {
        tracks: items,
        discovered: 2,
        failed: 1,
        unavailable: 1,
        ..Default::default()
    })]);
    let manager = manager(&storage, provider.clone());
    let done = finished(&manager, manager.start("fixture", "a").unwrap().job_id).await;
    assert_eq!(done.status, ImportStatus::Partial);
    assert_eq!(done.failed, 1);
    assert_eq!(done.unavailable, 1);
    assert_eq!(
        storage.favorites().unwrap()[0].availability,
        Availability::Unavailable
    );
    *provider.pages.lock().unwrap() = vec![Response::Repeat];
    let done = finished(&manager, manager.start("fixture", "a").unwrap().job_id).await;
    assert_eq!(done.status, ImportStatus::Partial);
    assert!(done.message.unwrap().contains("cursor"));
}
#[test]
fn version_one_migration_and_crash_recovery_keep_saved_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(include_str!("../../../migrations/001_init.sql"))
        .unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    drop(conn);
    let storage = Storage::open(&path).unwrap();
    let profile = ImportProfile {
        provider: "fixture".into(),
        provider_user_id: "1".into(),
        url: "https://soundcloud.com/a".into(),
        name: "A".into(),
        artwork: None,
    };
    let job = Uuid::new_v4();
    let id = storage.begin_import(&profile, job).unwrap();
    storage.save_import_page(id, job, tracks(2, 0)).unwrap();
    let p = ImportProgress {
        job_id: job,
        source_id: Some(id),
        profile: "A".into(),
        status: ImportStatus::Importing,
        discovered: 2,
        saved: 2,
        duplicates: 0,
        failed: 0,
        unavailable: 0,
        pages: 1,
        retry_at: None,
        message: None,
    };
    storage.save_import_progress(&p).unwrap();
    drop(storage);
    let storage = Storage::open(&path).unwrap();
    storage.recover_imports().unwrap();
    assert_eq!(storage.favorites().unwrap().len(), 2);
    assert_eq!(
        storage.import_source(id).unwrap().progress.unwrap().status,
        ImportStatus::Interrupted
    );
    assert!(storage.begin_import(&profile, Uuid::new_v4()).is_ok());
}
#[test]
fn detaching_source_keeps_tracks_and_provider_scoped_ids_never_collide() {
    let temp = tempfile::tempdir().unwrap();
    let storage = Storage::open(&temp.path().join("state.sqlite")).unwrap();
    let job = Uuid::new_v4();
    let profile = ImportProfile {
        provider: "fixture".into(),
        provider_user_id: "a".into(),
        url: "https://soundcloud.com/a".into(),
        name: "A".into(),
        artwork: None,
    };
    let source = storage.begin_import(&profile, job).unwrap();
    let track = tracks(1, 0).remove(0);
    let mut other = track.clone();
    other.internal_id = Uuid::new_v4();
    other.sources[0].provider = "future".into();
    storage
        .save_import_page(source, job, vec![track, other])
        .unwrap();
    assert_eq!(storage.favorites().unwrap().len(), 2);
    storage.remove_import_source(source, false).unwrap();
    assert_eq!(storage.local_favorite_ids().unwrap().len(), 2);
    assert!(storage.import_sources().unwrap().is_empty());
    let library = Library::load(&storage).unwrap();
    assert_eq!(library.favorites.len(), 2);
}
