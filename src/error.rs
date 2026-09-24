use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Kube API request failed: {0}")]
    Kube(#[from] kube::Error),

    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),
}