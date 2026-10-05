use reson_core::{
    audio::{diagnostics::inspect_pcm, AudioCommand, AudioEvent, NativeAudio},
    error::{Error, Result},
    providers::{soundcloud::SoundCloudProvider, MusicProvider},
};
use tokio_util::sync::CancellationToken;
pub async fn run() -> Result<()> {
    let provider = SoundCloudProvider::new()?;
    let results = provider
        .search("Scott Buckley", 0, CancellationToken::new())
        .await?;
    let track = results
        .tracks
        .into_iter()
        .find(|t| t.availability == reson_core::models::Availability::Playable)
        .ok_or(Error::Unavailable)?;
    let source = provider.resolve_stream(&track.sources[0]).await?;
    let root = std::env::temp_dir().join(format!("reson-smoke-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root)?;
    let path = root.join("audio.f32");
    let lib = if cfg!(target_os = "windows") {
        Some(
            std::env::current_exe()?
                .parent()
                .ok_or(Error::Malformed)?
                .join("libmpv-2.dll"),
        )
    } else {
        None
    };
    let (audio, mut events) = NativeAudio::start(lib, Some(path.clone()))?;
    audio.send(AudioCommand::Load {
        source,
        position_ms: 0,
        paused: false,
    })?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(120), async {
        loop {
            match events.recv().await {
                Some(AudioEvent::Position(p)) if p > 3000 => return Ok(()),
                Some(AudioEvent::Error(e)) => return Err(Error::Audio(e)),
                None => return Err(Error::Audio("Audio worker stopped".into())),
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| Err(Error::Audio("Native audio validation timed out".into())));
    let _ = audio.send(AudioCommand::Stop);
    let _ = audio.send(AudioCommand::Shutdown);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let evidence = inspect_pcm(&path);
    let _ = std::fs::remove_dir_all(&root);
    result?;
    let evidence = evidence?;
    println!(
        "Real SoundCloud search → track → refreshed source → native PCM output: {} samples, RMS {:.6}, peak {:.6}", evidence.samples, evidence.rms, evidence.peak
    );
    Ok(())
}
