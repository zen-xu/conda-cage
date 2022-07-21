use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("path: {path}")]
    IoError {
        #[source]
        source: std::io::Error,
        path: PathBuf,
    },
}

pub(crate) trait IoResultExt<T> {
    fn with_err_path<F, P>(self, path: F) -> Result<T, Error>
    where
        F: FnOnce() -> P,
        P: Into<PathBuf>;
}

impl<T> IoResultExt<T> for Result<T, io::Error> {
    fn with_err_path<F, P>(self, path: F) -> Result<T, Error>
    where
        F: FnOnce() -> P,
        P: Into<PathBuf>,
    {
        self.map_err(|e| Error::IoError {
            source: io::Error::new(e.kind(), e),
            path: path().into(),
        })
    }
}
