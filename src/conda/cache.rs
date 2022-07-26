use std::{io::Read, path::PathBuf};

use buffered_reader::Memory;

use super::tarball_name;
use crate::{error::IoResultExt, CondaInfo, Package, Result};

#[derive(Debug)]
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
            self.packages_dir.join(tarball_name(
                &pkg.data.name,
                &pkg.data.version,
                &pkg.data.build,
            )),
            tarball_data,
        )?;

        Ok(self.get_tarball(pkg).unwrap())
    }

    pub fn unpack_tarball(&self, pkg: &Package) -> Result<()> {
        let tarball = self
            .get_tarball(pkg)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, ""))
            .with_err_path(|| {
                self.packages_dir.join(tarball_name(
                    &pkg.data.name,
                    &pkg.data.version,
                    &pkg.data.build,
                ))
            })?;

        let unpack_dir = self.packages_dir.join(
            tarball
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches(".tar.bz2"),
        );
        if unpack_dir.exists() {
            std::fs::remove_dir_all(&unpack_dir)?;
        }
        std::fs::create_dir_all(&unpack_dir)?;
        let file = std::fs::File::open(tarball)?;
        let mut decoder = bzip2::read::BzDecoder::new(file);
        let mut data = vec![];
        decoder.read_to_end(&mut data)?;
        let buf = Memory::new(&data);
        tar::Archive::new(buf).unpack(&unpack_dir)?;

        Ok(())
    }
}

#[inline]
fn extracted_dir(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}")
}
