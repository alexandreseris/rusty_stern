use thiserror::Error;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("file `{0}` not found")]
    FileNotFound(String),
    #[error("error on file `{0}`: `{1}`")]
    FileError(String, String),
    #[error("`{0}`")]
    Validation(String),
    #[error("`{0}`")]
    LogError(String),
    #[error("`{0}`")]
    StdErr(String),
    #[error("api failled for `{0}`, detail: `{1}`")]
    Kubernetes(String, String),
    #[error("`{0}`")]
    Other(String),
}
