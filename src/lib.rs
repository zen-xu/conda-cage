mod conda;
mod error;

pub use conda::{CondaInfo, PackageData, RepoIndex};
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
