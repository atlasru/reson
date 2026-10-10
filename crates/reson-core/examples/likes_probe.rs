use reson_core::{
    error::{Error, Result},
    library::imports::{ImportManager, ImportStatus},
    providers::{soundcloud::SoundCloudProvider, ProviderRegistry},
    storage::Storage,
};
use std::{sync::Arc, time::Duration};
#[tokio::main]
async fn main() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("likes.sqlite");
    let storage = Storage::open(&path)?;
    let mut registry = ProviderRegistry::default();
    registry.register(Arc::new(SoundCloudProvider::new()?));
    let manager = ImportManager::new(storage.clone(), registry);
    let mut events = manager.subscribe();
    let job = manager.start(
        "soundcloud",
        &std::env::args()
            .nth(1)
            .unwrap_or_else(|| "scottbuckley".into()),
    )?;
    let done = tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let p = events.recv().await.map_err(|_| Error::Cancelled)?;
            if p.job_id == job.job_id && !p.status.active() {
                return Ok::<_, Error>(p);
            }
        }
    })
    .await
    .map_err(|_| Error::Invalid("Live import timed out".into()))??;
    println!("Profile: {}. Status: {:?}. Pages: {}. Discovered: {}. Saved: {}. Duplicates: {}. Failed: {}. Unavailable: {}.",done.profile,done.status,done.pages,done.discovered,done.saved,done.duplicates,done.failed,done.unavailable);
    if done.status != ImportStatus::Complete {
        return Err(Error::Invalid(
            done.message
                .unwrap_or_else(|| "Import was incomplete".into()),
        ));
    }
    let before = storage.favorites()?.len();
    drop(storage);
    let reopened = Storage::open(&path)?;
    if reopened.favorites()?.len() != before {
        return Err(Error::Invalid("Restart persistence failed".into()));
    }
    println!("Restart persistence verified: {before} tracks.");
    Ok(())
}
