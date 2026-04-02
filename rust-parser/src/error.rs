use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CEF parse error: {0}")]
    Cef(String),

    #[error("Syslog parse error: {0}")]
    Syslog(String),

    #[error("Unknown format: cannot detect log type")]
    UnknownFormat,

    #[error("Event too large: {size} bytes (max {max})")]
    EventTooLarge { size: usize, max: usize },

    #[error("Enrichment error: {0}")]
    Enrichment(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
