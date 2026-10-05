use reson_core::error::Result;
use std::path::Path;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
pub fn initialize(dir: &Path) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    std::fs::create_dir_all(dir)?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("reson")
        .filename_suffix("jsonl")
        .max_log_files(3)
        .build(dir)
        .map_err(|_| {
            reson_core::error::Error::Invalid("Cannot initialize application logs".into())
        })?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let level = match std::env::var("RESON_LOG").as_deref() {
        Ok("debug") => "debug",
        _ => "info",
    };
    let filter =
        tracing_subscriber::EnvFilter::new(format!("warn,reson={level},reson_core={level}"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(writer)
                .with_ansi(false),
        )
        .try_init();
    Ok(guard)
}
pub async fn export(logs: &Path, data_dir: &Path, audio_error: Option<String>) -> Result<String> {
    let root = data_dir.join("diagnostics");
    tokio::fs::create_dir_all(&root).await?;
    let destination = root.join(format!(
        "reson-diagnostics-{}.txt",
        reson_core::storage::now()
    ));
    let mut report=format!("Reson {}\nPlatform: {}\nAudio initialization: {}\nNo credentials, stream URLs, searches or library data are included.\n\n",env!("CARGO_PKG_VERSION"),std::env::consts::OS,audio_error.unwrap_or_else(||"OK".into()));
    let mut files = tokio::fs::read_dir(logs).await?;
    let mut remaining = 2_000_000usize;
    while let Some(file) = files.next_entry().await? {
        if !file.file_type().await?.is_file() {
            continue;
        }
        let body = tokio::fs::read(file.path()).await?;
        let body = &body[body.len().saturating_sub(remaining.min(256000))..];
        report.push_str(&String::from_utf8_lossy(body));
        remaining = remaining.saturating_sub(body.len());
        if remaining == 0 {
            break;
        }
    }
    tokio::fs::write(&destination, report).await?;
    Ok(destination.to_string_lossy().into_owned())
}
