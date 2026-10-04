// file: app/src/error.rs
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub kind: String,
    pub message: String,
}

impl CommandError {
    #[must_use]
    pub fn from_core(e: kryvora_core::Error) -> Self {
        Self {
            kind: format!("{:?}", e.kind()).to_lowercase(),
            message: e.to_string(),
        }
    }
}

impl From<kryvora_core::Error> for CommandError {
    fn from(e: kryvora_core::Error) -> Self {
        Self::from_core(e)
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        Self {
            kind: "io_error".into(),
            message: e.to_string(),
        }
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(error: rusqlite::Error) -> Self {
        Self {
            kind: "database_error".into(),
            message: error.to_string(),
        }
    }
}
