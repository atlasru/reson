use crate::{
    error::{Error, Result},
    models::Track,
    providers::{Capability, MusicProvider, ProviderRegistry},
    storage::Storage,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProfile {
    pub provider: String,
    pub provider_user_id: String,
    pub url: String,
    pub name: String,
    pub artwork: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LikesPage {
    pub tracks: Vec<Track>,
    pub discovered: usize,
    pub failed: usize,
    pub unavailable: usize,
    pub next: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Resolving,
    Importing,
    Cooldown,
    Complete,
    Partial,
    Cancelled,
    Failed,
    Interrupted,
}
impl ImportStatus {
    pub fn active(self) -> bool {
        matches!(self, Self::Resolving | Self::Importing | Self::Cooldown)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub job_id: Uuid,
    pub source_id: Option<Uuid>,
    pub profile: String,
    pub status: ImportStatus,
    pub discovered: usize,
    pub saved: usize,
    pub duplicates: usize,
    pub failed: usize,
    pub unavailable: usize,
    pub pages: usize,
    pub retry_at: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSource {
    pub id: Uuid,
    #[serde(flatten)]
    pub profile: ImportProfile,
    pub created_at: i64,
    pub refreshed_at: Option<i64>,
    pub track_count: usize,
    pub progress: Option<ImportProgress>,
}

struct Job {
    input: String,
    provider: String,
    cancel: CancellationToken,
    progress: ImportProgress,
}
#[derive(Clone)]
pub struct ImportManager {
    storage: Storage,
    providers: ProviderRegistry,
    jobs: Arc<Mutex<HashMap<Uuid, Job>>>,
    events: broadcast::Sender<ImportProgress>,
}
impl ImportManager {
    pub fn new(storage: Storage, providers: ProviderRegistry) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            storage,
            providers,
            jobs: Default::default(),
            events,
        }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<ImportProgress> {
        self.events.subscribe()
    }
    pub fn progress(&self) -> Vec<ImportProgress> {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .map(|j| j.progress.clone())
            .collect()
    }
    pub fn start(&self, provider_id: &str, input: &str) -> Result<ImportProgress> {
        let provider = self.providers.get(provider_id)?;
        if !provider
            .info()
            .capabilities
            .contains(&Capability::PublicLikesImport)
        {
            return Err(Error::Unsupported("public likes import"));
        }
        let input = provider.normalize_profile_input(input)?;
        let progress = ImportProgress {
            job_id: Uuid::new_v4(),
            source_id: None,
            profile: input.clone(),
            status: ImportStatus::Resolving,
            discovered: 0,
            saved: 0,
            duplicates: 0,
            failed: 0,
            unavailable: 0,
            pages: 0,
            retry_at: None,
            message: None,
        };
        let cancel = CancellationToken::new();
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            if jobs.values().any(|j| {
                j.provider == provider_id && j.input == input && j.progress.status.active()
            }) {
                return Err(Error::Invalid(
                    "This profile is already being imported".into(),
                ));
            }
            if jobs.values().filter(|j| j.progress.status.active()).count() >= 4 {
                return Err(Error::Invalid("Four imports are already running".into()));
            }
            // Keep recent terminal jobs for dialogs, while bounding session memory.
            if jobs.len() >= 32 {
                jobs.retain(|_, j| j.progress.status.active());
            }
            jobs.insert(
                progress.job_id,
                Job {
                    input: input.clone(),
                    provider: provider_id.into(),
                    cancel: cancel.clone(),
                    progress: progress.clone(),
                },
            );
        }
        let manager = self.clone();
        let initial = progress.clone();
        tokio::spawn(async move {
            manager.run(provider, input, cancel, initial).await;
        });
        Ok(progress)
    }
    pub fn cancel(&self, job_id: Uuid) {
        if let Some(job) = self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&job_id)
        {
            job.cancel.cancel();
        }
    }
    pub fn cancel_source(&self, source_id: Uuid) {
        let source = self.storage.import_source(source_id).ok();
        for job in self.jobs.lock().unwrap_or_else(|e| e.into_inner()).values() {
            let resolving_source = source.as_ref().is_some_and(|s| {
                job.provider == s.profile.provider
                    && (job.input == s.profile.provider_user_id
                        || self
                            .providers
                            .get(&job.provider)
                            .and_then(|p| p.normalize_profile_input(&s.profile.url))
                            .is_ok_and(|url| url == job.input))
            });
            if job.progress.source_id == Some(source_id) || resolving_source {
                job.cancel.cancel();
            }
        }
    }
    fn publish(&self, progress: &ImportProgress) {
        if let Some(job) = self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&progress.job_id)
        {
            job.progress = progress.clone();
        }
        let _ = self.events.send(progress.clone());
    }
    async fn persist(&self, progress: &ImportProgress) -> Result<()> {
        let storage = self.storage.clone();
        let p = progress.clone();
        tokio::task::spawn_blocking(move || storage.save_import_progress(&p))
            .await
            .map_err(|_| Error::Invalid("Import storage task interrupted".into()))?
    }
    async fn cooldown(
        &self,
        seconds: u64,
        cancel: &CancellationToken,
        p: &mut ImportProgress,
    ) -> Result<()> {
        p.status = ImportStatus::Cooldown;
        p.retry_at =
            Some(crate::storage::now().saturating_add(seconds.min(i64::MAX as u64) as i64));
        p.message = Some("SoundCloud rate limit. Waiting to retry.".into());
        self.persist(p).await?;
        self.publish(p);
        tokio::select! { biased; _=cancel.cancelled()=>Err(Error::Cancelled), _=tokio::time::sleep(Duration::from_secs(seconds))=>Ok(()) }
    }
    async fn run(
        &self,
        provider: Arc<dyn MusicProvider>,
        input: String,
        cancel: CancellationToken,
        mut p: ImportProgress,
    ) {
        self.publish(&p);
        let result = self.collect(provider, input, &cancel, &mut p).await;
        p.retry_at = None;
        match result {
            Ok(()) => {
                p.status = if p.failed > 0 {
                    ImportStatus::Partial
                } else {
                    ImportStatus::Complete
                };
                p.message = if p.failed > 0 {
                    Some("All accessible pages processed; some track entries could not be read. Refresh to retry.".into())
                } else {
                    None
                };
            }
            Err(Error::Cancelled) => {
                p.status = ImportStatus::Cancelled;
                p.message = Some("Import cancelled. Saved tracks were kept.".into());
            }
            Err(error) => {
                p.status = if p.pages > 0 {
                    ImportStatus::Partial
                } else {
                    ImportStatus::Failed
                };
                p.message = Some(error.to_string());
            }
        }
        if let Err(error) = self.persist(&p).await {
            p.status = ImportStatus::Failed;
            p.message = Some(error.to_string());
        }
        self.publish(&p);
    }
    async fn collect(
        &self,
        provider: Arc<dyn MusicProvider>,
        input: String,
        cancel: &CancellationToken,
        p: &mut ImportProgress,
    ) -> Result<()> {
        let profile = loop {
            let result = tokio::select! { biased; _=cancel.cancelled()=>return Err(Error::Cancelled), result=provider.public_profile(&input, cancel.clone())=>result };
            match result {
                Err(Error::RateLimited(s)) => self.cooldown(s, cancel, p).await?,
                result => break result?,
            }
        };
        let storage = self.storage.clone();
        let job = p.job_id;
        let begin_cancel = cancel.clone();
        p.profile = profile.name.clone();
        p.source_id = Some(
            tokio::task::spawn_blocking(move || {
                storage.begin_import_checked(&profile, job, || !begin_cancel.is_cancelled())
            })
            .await
            .map_err(|_| Error::Invalid("Import storage task interrupted".into()))??,
        );
        let mut cursor: Option<String> = None;
        let mut seen = HashSet::new();
        loop {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            p.status = ImportStatus::Importing;
            p.retry_at = None;
            p.message = None;
            self.publish(p);
            let page = loop {
                let source = self
                    .storage
                    .import_source(p.source_id.expect("resolved profile"))?;
                let result = tokio::select! { biased; _=cancel.cancelled()=>return Err(Error::Cancelled), result=provider.liked_tracks_page(&source.profile.provider_user_id, cursor.as_deref(), cancel.clone())=>result };
                match result {
                    Err(Error::RateLimited(s)) => self.cooldown(s, cancel, p).await?,
                    result => break result?,
                }
            };
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            p.status = ImportStatus::Importing;
            p.retry_at = None;
            p.message = None;
            p.discovered += page.discovered;
            p.failed += page.failed;
            p.unavailable += page.unavailable;
            p.pages += 1;
            let storage = self.storage.clone();
            let mut snapshot = p.clone();
            let saved = tokio::task::spawn_blocking(move || {
                let (added, duplicates) = storage.save_import_page(
                    snapshot.source_id.expect("resolved profile"),
                    snapshot.job_id,
                    page.tracks,
                )?;
                snapshot.saved += added;
                snapshot.duplicates += duplicates;
                storage.save_import_progress(&snapshot)?;
                Ok::<_, Error>(snapshot)
            })
            .await
            .map_err(|_| Error::Invalid("Import storage task interrupted".into()))??;
            *p = saved;
            self.publish(p);
            match page.next {
                None => return Ok(()),
                Some(next) => {
                    if !seen.insert(next.clone()) {
                        return Err(Error::Invalid("SoundCloud repeated a pagination cursor. Import is incomplete; refresh to retry.".into()));
                    }
                    cursor = Some(next);
                }
            }
        }
    }
}
