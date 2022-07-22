mod index;
mod info;

pub use index::{CondaIndex, Package, PackageData};
pub use info::CondaInfo;

#[inline]
fn tarball_name(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}.tar.bz2")
}
