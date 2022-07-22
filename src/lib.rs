mod conda;
mod error;

pub use conda::CondaInfo;
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
