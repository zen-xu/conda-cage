mod conda;
mod error;

pub use conda::{CondaIndex, CondaInfo, Package, PackageData};
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
