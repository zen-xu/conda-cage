use std::{io::Read, path::PathBuf};

use buffered_reader::Memory;
use serde::{Deserialize, Serialize};

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

    pub fn try_get_prefix_record(&self, pkg: &Package) -> Result<PrefixRecord> {
        let extracted_dir = {
            let extracted_dir = self.get_extracted_dir(pkg);
            if extracted_dir.is_none() {
                self.unpack_tarball(pkg)?;
            }
            self.get_extracted_dir(pkg).unwrap()
        };

        let paths_path = extracted_dir.join("info/paths.json");
        let paths_data_contents = std::fs::read(&paths_path).with_err_path(|| paths_path)?;
        let paths_data = serde_json::from_slice::<PathsData>(&paths_data_contents)?;

        let repodata_record_path = extracted_dir.join("info/repodata_record.json");
        let repodata_record = if repodata_record_path.exists() {
            let repodata_record_contents =
                std::fs::read(&repodata_record_path).with_err_path(|| repodata_record_path)?;
            serde_json::from_slice::<PackageRecord>(&repodata_record_contents)?
        } else {
            let filename = self
                .get_tarball(pkg)
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let repodata_record = PackageRecord {
                name: pkg.data.name.clone(),
                version: pkg.data.version.clone(),
                r#fn: filename.clone(),
                build: pkg.data.build.clone(),
                build_number: pkg.data.build_number,
                channel: pkg.channel_url.clone(),
                subdir: pkg.subdir.clone(),
                md5: pkg.data.md5.clone(),
                legacy_bz2_md5: pkg.data.md5.clone(),
                legacy_bz2_size: pkg.data.size,
                url: Some(format!("{}/{}", pkg.channel_url, filename)),
                sha256: pkg.data.sha256.clone(),
                metadata_signature_status: None,
                arch: pkg.data.arch.clone(),
                platform: pkg.data.platform.clone(),
                depends: pkg.data.depends.clone(),
                constrains: vec![],
                track_features: "".to_string(),
                features: "".to_string(),
                noarch: None,
                preferred_env: None,
                license: pkg.data.license.clone(),
                license_family: pkg.data.license_family.clone(),
                timestamp: pkg.data.timestamp,
                date: None,
                size: pkg.data.size,
            };

            std::fs::write(
                repodata_record_path,
                serde_json::to_string(&repodata_record)?,
            )?;

            repodata_record
        };

        Ok(PrefixRecord {
            repodata_record,
            package_tarball_full_path: self.get_tarball(pkg).map(|p| p.display().to_string()),
            extracted_package_dir: self.get_extracted_dir(pkg).map(|p| p.display().to_string()),
            files: paths_data.paths.iter().map(|p| p.path.clone()).collect(),
            paths_data,
            link: Some(Link {
                source: self
                    .get_extracted_dir(pkg)
                    .map(|p| p.display().to_string())
                    .unwrap(),
                link_type: 1,
            }),
            requested_spec: None,
            auth: None,
        })
    }
}

#[inline]
fn extracted_dir(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}")
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageRecord {
    name: String,
    version: String,
    r#fn: String,
    #[serde(alias = "build_string")]
    build: String,
    build_number: u64,
    channel: String,
    subdir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_bz2_md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_bz2_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_signature_status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<String>,
    #[serde(default)]
    depends: Vec<String>,
    #[serde(default)]
    constrains: Vec<String>,
    track_features: String,
    #[serde(default)]
    features: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    noarch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license_family: Option<String>,
    timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrefixRecord {
    #[serde(flatten)]
    repodata_record: PackageRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_tarball_full_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extracted_package_dir: Option<String>,
    #[serde(default)]
    files: Vec<String>,
    paths_data: PathsData,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_spec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<String>,
}

impl PrefixRecord {
    pub fn paths(&self) -> Vec<&PathData> {
        self.paths_data.paths.iter().collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PathsData {
    #[serde(default)]
    paths_version: u32,
    #[serde(default)]
    paths: Vec<PathData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathData {
    #[serde(rename = "_path")]
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix_placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_mode: Option<FileMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    no_link: Option<bool>,
    path_type: PathType,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_in_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inode_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256_in_prefix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FileMode {
    Text,
    Binary,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PathType {
    HardLink,
    SoftLink,
    Directory,
}

#[derive(Debug, Serialize, Deserialize)]
struct Link {
    source: String,
    /// hardlink 1
    /// softlink 2
    /// copy 3
    /// directory 4
    #[serde(rename = "type")]
    link_type: u8,
}

#[test]
fn test_prefix_record() {
    use assert_json_diff::assert_json_eq;
    use serde_json::Value;
    use std::fs;

    let contents = fs::read_to_string("fixtures/libcxx-12.0.0-h2f01273_0.json").unwrap();
    let record = serde_json::from_str::<PrefixRecord>(&contents).unwrap();
    let deserialized = serde_json::to_string(&record).unwrap();
    let expected = serde_json::from_str::<Value>(&contents).unwrap();
    let actual = serde_json::from_str::<Value>(&deserialized).unwrap();

    assert_json_eq!(actual, expected);
}

#[test]
fn test_get_prefix_record() {
    use super::CondaIndex;

    let info = CondaInfo::try_new("conda").unwrap();
    let cache = CondaCache::new(&info);
    let index = CondaIndex::try_new(&info, &cache, vec!["pkgs/main".to_string()]).unwrap();
    let pkg = index.get("xz", "5.2.5", "h1de35cc_0").unwrap();
    index.download(&pkg).unwrap();
    let prefix_record = cache.try_get_prefix_record(&pkg).unwrap();
    println!("{:#?}", prefix_record)
}
