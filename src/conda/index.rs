use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::CondaInfo;
use crate::{error::IoResultExt, Error, Result};

#[derive(Deserialize)]
struct IndexData {
    packages: HashMap<String, PackageData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageData {
    pub size: u64,
    pub timestamp: Option<u64>,
    pub source_url: Option<String>,
    pub depends: Vec<String>,
    pub arch: Option<String>,
    pub md5: String,
    pub build_number: u64,
    pub name: String,
    pub license: Option<String>,
    pub license_family: Option<String>,
    pub platform: Option<String>,
    pub version: String,
    pub subdir: String,
    pub sha256: String,
    pub build: String,
}

#[derive(Debug)]
pub struct CondaIndex {
    info: CondaInfo,
    // channel -> subdir -> repo data
    indexes: HashMap<String, HashMap<String, HashMap<String, PackageData>>>,
    cache_dir: PathBuf,
}

impl CondaIndex {
    pub fn try_new<P: Into<PathBuf>>(
        info: CondaInfo,
        cache_dir: P,
        channels: Vec<String>,
    ) -> Result<Self> {
        let cache_dir: PathBuf = cache_dir.into();
        let mut indexes: HashMap<String, HashMap<String, HashMap<String, PackageData>>> =
            HashMap::new();

        for channel in channels.iter() {
            for subdir in info.subdirs.iter() {
                let cached_path = cached_index_path(&cache_dir, channel, subdir);
                if !cached_path.exists() {
                    update_cached_indexes(&cache_dir, &info, vec![channel])?;
                }

                let sub_indexes = load_cached_index(&cache_dir, channel, subdir)?;
                indexes
                    .entry(channel.to_string())
                    .or_insert_with(HashMap::new)
                    .insert(subdir.to_string(), sub_indexes);
            }
        }

        Ok(Self {
            info,
            indexes,
            cache_dir,
        })
    }

    /// get repo data, first value is tarball name, second value is repo data
    pub fn get(
        &self,
        channel: &str,
        name: &str,
        version: &str,
        build: &str,
    ) -> Option<(String, &PackageData)> {
        for (_, subdir_indexes) in self.indexes.get(channel)?.iter() {
            let tarball_name = tarball_name(name, version, build);
            let repo_data = subdir_indexes.get(&tarball_name);
            if let Some(repo_data) = repo_data {
                return Some((tarball_name, repo_data));
            }
        }

        None
    }

    /// update indexes by the given channels
    pub fn update_indexes(&mut self, channels: Vec<&str>) -> Result<()> {
        update_cached_indexes(&self.cache_dir, &self.info, channels.clone())?;

        for channel in channels.into_iter() {
            for subdir in self.info.subdirs.iter() {
                let sub_indexes = load_cached_index(&self.cache_dir, channel, subdir)?;
                self.indexes
                    .entry(channel.to_string())
                    .or_insert_with(HashMap::new)
                    .insert(subdir.to_string(), sub_indexes);
            }
        }

        Ok(())
    }
}

fn update_cached_indexes(
    cache_dir: &Path,
    conda_info: &CondaInfo,
    channels: Vec<&str>,
) -> Result<()> {
    if !cache_dir.exists() {
        std::fs::create_dir_all(cache_dir)?;
    }

    for channel in channels {
        for subdir in conda_info.subdirs.iter() {
            let url = url::Url::parse(&format!(
                "{}/{}/{}/repodata.json",
                conda_info.channel_alias.trim_end_matches('/'),
                channel,
                subdir
            ))
            .unwrap();
            let rsp = reqwest::blocking::get(url.clone()).map_err(|e| e.with_url(url.clone()))?;
            if !rsp.status().is_success() {
                return Err(Error::OtherError(format!(
                    "fail to fetch {}, code: {}",
                    url,
                    rsp.status()
                )));
            }
            let cache_path = cached_index_path(cache_dir, channel, subdir);
            std::fs::write(&cache_path, rsp.bytes()?).with_err_path(|| cache_path)?;
        }
    }
    Ok(())
}

fn cached_index_path(cache_dir: &Path, channel: &str, subdir: &str) -> PathBuf {
    let channel = channel.replace('/', "_");
    cache_dir.join(format!("{channel}_{subdir}"))
}

fn load_cached_index(
    cache_dir: &Path,
    channel: &str,
    subdir: &str,
) -> Result<HashMap<String, PackageData>> {
    let data = std::fs::read(cached_index_path(cache_dir, channel, subdir))?;
    Ok(serde_json::from_slice::<IndexData>(&data)?.packages)
}

#[inline]
fn tarball_name(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}.tar.bz2")
}
