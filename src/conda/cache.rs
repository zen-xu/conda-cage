use std::path::PathBuf;

use super::tarball_name;
use crate::{CondaInfo, Package, Result};

pub struct CondaCache {
    pub packages_dir: PathBuf,
}

impl CondaCache {
    pub fn new(conda_info: &CondaInfo) -> Self {
        Self {
            packages_dir: PathBuf::from(conda_info.root_prefix.clone()).join("pkgs"),
        }
    }

    pub fn get_extracted_dir(&self, pkg: &Package) -> Option<PathBuf> {
        let pkg_dir = self.packages_dir.join(extracted_dir(
            &pkg.data.name,
            &pkg.data.version,
            &pkg.data.build,
        ));

        if pkg_dir.exists() {
            Some(pkg_dir)
        } else {
            None
        }
    }

    pub fn get_tarball(&self, pkg: &Package) -> Option<PathBuf> {
        let tarball_path = self.packages_dir.join(tarball_name(
            &pkg.data.name,
            &pkg.data.version,
            &pkg.data.build,
        ));

        if tarball_path.exists() {
            Some(tarball_path)
        } else {
            None
        }
    }

    pub fn add_tarball<D: AsRef<[u8]>>(&self, pkg: &Package, tarball_data: D) -> Result<PathBuf> {
        std::fs::write(
            tarball_name(&pkg.data.name, &pkg.data.version, &pkg.data.build),
            tarball_data,
        )?;

        Ok(self.get_tarball(pkg).unwrap())
    }
}

#[inline]
fn extracted_dir(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}")
}
