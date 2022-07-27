use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::{recipe::Spec, tarball_name, CondaCache, CondaInfo};
use crate::{error::IoResultExt, Error, Result};

#[derive(Deserialize)]
struct IndexData {
    packages: HashMap<String, PackageData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageData {
    pub size: Option<usize>,
    pub timestamp: Option<u64>,
    pub source_url: Option<String>,
    pub depends: Vec<String>,
    pub arch: Option<String>,
    pub md5: Option<String>,
    pub build_number: u64,
    pub name: String,
    pub license: Option<String>,
    pub license_family: Option<String>,
    pub platform: Option<String>,
    pub version: String,
    pub subdir: String,
    pub sha256: Option<String>,
    pub build: String,
}

pub struct Package {
    pub tarball_name: String,
    pub data: PackageData,
    pub channel: String,
    pub subdir: String,
    pub channel_url: String,
}

#[derive(Debug)]
pub struct CondaIndex<'i, 'c> {
    info: &'i CondaInfo,
    cache: &'c CondaCache,
    // channel -> subdir -> repo data
    indexes: HashMap<String, HashMap<String, HashMap<String, PackageData>>>,
    cache_dir: PathBuf,
}

impl<'i, 'c> CondaIndex<'i, 'c> {
    pub fn try_new<'s, I, S>(
        info: &'i CondaInfo,
        cache: &'c CondaCache,
        channels: I,
    ) -> Result<Self>
    where
        I: IntoIterator<Item = &'s S>,
        S: AsRef<str> + 's,
    {
        let cache_dir = cache.packages_dir.join("cache");

        let mut indexes: HashMap<String, HashMap<String, HashMap<String, PackageData>>> =
            HashMap::new();

        for channel in channels.into_iter().map(AsRef::as_ref) {
            for subdir in info.subdirs.iter() {
                let cached_path = cached_index_path(&cache_dir, channel, subdir);
                if !cached_path.exists() {
                    update_cached_indexes(&cache_dir, &info, &[channel])?;
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
            cache,
        })
    }

    /// get package data
    pub fn get(&self, name: &str, version: &str, build: &str) -> Option<Package> {
        for (channel, channel_indexes) in self.indexes.iter() {
            for (subdir, subdir_indexes) in channel_indexes.iter() {
                let tarball_name = tarball_name(name, version, build);
                let repo_data = subdir_indexes.get(&tarball_name);
                if let Some(repo_data) = repo_data {
                    return Some(Package {
                        tarball_name,
                        data: repo_data.clone(),
                        channel: channel.clone(),
                        channel_url: format!(
                            "{}/{}",
                            self.info.channel_alias.trim_end_matches('/'),
                            channel
                        ),
                        subdir: subdir.clone(),
                    });
                }
            }
        }

        None
    }

    /// get package by spec
    pub fn get_by_spec(&self, spec: &Spec) -> Option<Package> {
        self.get(&spec.name, &spec.version, &spec.build)
    }

    // download pkg tarball and unpack it
    pub fn download(&self, pkg: &Package) -> Result<()> {
        let url = url::Url::parse(&format!(
            "{}/{}/{}/{}",
            self.info.channel_alias.trim_end_matches('/'),
            pkg.channel,
            pkg.data.subdir,
            pkg.tarball_name,
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

        self.cache.add_tarball(pkg, &rsp.bytes()?)?;
        self.cache.unpack_tarball(pkg)?;

        Ok(())
    }

    /// update indexes by the given channels
    pub fn update_indexes<'s, I, S>(&mut self, channels: I) -> Result<()>
    where
        I: IntoIterator<Item = &'s S>,
        S: AsRef<str> + 's,
    {
        let channels = channels
            .into_iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>();
        update_cached_indexes(&self.cache_dir, &self.info, &channels)?;

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

fn update_cached_indexes<'s, I, S>(
    cache_dir: &Path,
    conda_info: &CondaInfo,
    channels: I,
) -> Result<()>
where
    I: IntoIterator<Item = &'s S>,
    S: AsRef<str> + 's,
{
    if !cache_dir.exists() {
        std::fs::create_dir_all(cache_dir)?;
    }

    for channel in channels.into_iter().map(AsRef::as_ref) {
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

/* #[test]
fn test_download() {
    let info = CondaInfo::try_new("conda").unwrap();
    let cache = CondaCache::new(&info);
    let index = CondaIndex::try_new(&info, &cache, vec!["pkgs/main".to_string()]).unwrap();
    let pkg = index.get("xz", "5.2.5", "h1de35cc_0").unwrap();
    index.download(&pkg).unwrap();
} */
