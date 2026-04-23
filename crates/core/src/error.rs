use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
 #[error("I/O error: {0}")]
 Io(#[from] std::io::Error),
 #[error("configuration error: {0}")]
 Config(String),
 #[error("invalid state: {0}")]
 InvalidState(String),
 #[error("invalid data: {0}")]
 InvalidData(String),
}

pub type AppResult<T> = Result<T, AppError>;
