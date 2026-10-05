pub mod diagnostics;
mod mpv;

use crate::{error::Result, models::StreamSource};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AudioEvent {
    Loaded,
    Position(u64),
    Duration(u64),
    Paused(bool),
    Buffering(bool),
    Ended,
    Error(String),
}
#[derive(Debug, Clone, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}
#[derive(Clone)]
pub enum AudioCommand {
    Load {
        source: StreamSource,
        position_ms: u64,
        paused: bool,
    },
    Pause(bool),
    Stop,
    Seek(u64),
    Volume(f64),
    Device(String),
    Shutdown,
}
pub trait AudioBackend: Send + Sync {
    fn send(&self, command: AudioCommand) -> Result<()>;
    fn devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(vec![AudioDevice {
            id: "auto".into(),
            name: "System default".into(),
        }])
    }
}
pub struct NativeAudio;
impl NativeAudio {
    pub fn start(
        library: Option<PathBuf>,
        output: Option<PathBuf>,
    ) -> Result<(Arc<dyn AudioBackend>, mpsc::UnboundedReceiver<AudioEvent>)> {
        mpv::start(library, output)
    }
}
