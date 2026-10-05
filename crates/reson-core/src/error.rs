use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Network request failed. Check your connection and try again.")]
    Network,
    #[error("SoundCloud is rate limiting requests. Try again in {0} seconds.")]
    RateLimited(u64),
    #[error("This content is unavailable in your region or has been removed.")]
    Unavailable,
    #[error("This provider does not support {0}.")]
    Unsupported(&'static str),
    #[error("The provider returned an invalid response.")]
    Malformed,
    #[error("Request cancelled.")]
    Cancelled,
    #[error("Invalid input: {0}")]
    Invalid(String),
    #[error("Audio error: {0}")]
    Audio(String),
    #[error("Storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unable to encode local state.")]
    Json(#[from] serde_json::Error),
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;
