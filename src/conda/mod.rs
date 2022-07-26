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

use serde::{Deserialize, Serialize};

/// when installed conda pkg, should dump this prefix record in conda-meta directory
#[derive(Debug, Serialize, Deserialize)]
pub struct PrefixRecord {
    pub name: String,
    pub version: String,
    #[serde(alias = "fn")]
    #[serde(rename(serialize = "fn"))]
    pub filename: String,
    #[serde(alias = "build_string")]
    pub build: String,
    pub build_number: u32,
    pub channel: String,
    pub subdir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_bz2_md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_bz2_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_signature_status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default)]
    pub depends: Vec<String>,
    #[serde(default)]
    pub constrains: Vec<String>,
    pub track_features: String,
    #[serde(default)]
    pub features: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noarch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_family: Option<String>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_tarball_full_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_package_dir: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    pub paths_data: PathsData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_spec: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathsData {
    #[serde(default)]
    pub paths_version: u32,
    #[serde(default)]
    pub paths: Vec<PathData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathData {
    #[serde(rename = "_path")]
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_mode: Option<FileMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_link: Option<bool>,
    pub path_type: PathType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_in_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inode_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256_in_prefix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileMode {
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
pub struct Link {
    pub source: String,
    #[serde(rename = "type")]
    pub link_type: u8,
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
