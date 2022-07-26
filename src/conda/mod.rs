pub mod cache;
pub mod index;
pub mod info;
pub mod recipe;

pub use cache::CondaCache;
pub use index::{CondaIndex, Package, PackageData};
pub use info::CondaInfo;
pub use recipe::CondaRecipe;

#[inline]
fn tarball_name(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}.tar.bz2")
}
