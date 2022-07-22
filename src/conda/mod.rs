mod cache;
mod index;
mod info;

pub use cache::CondaCache;
pub use index::{CondaIndex, Package, PackageData};
pub use info::CondaInfo;

use crate::{Error, Result};

#[inline]
fn tarball_name(name: &str, version: &str, build: &str) -> String {
    format!("{name}-{version}-{build}.tar.bz2")
}

#[derive(Debug)]
pub struct CondaRecipe {
    specs: Vec<Spec>,
}

#[derive(Debug)]
struct Spec {
    name: String,
    version: String,
    build: String,
    channel: String,
}

impl CondaRecipe {
    pub fn try_new(content: &str) -> Result<Self> {
        let mut specs = vec![];

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("#") || line.is_empty() {
                continue;
            }

            let mut splitted = line
                .split_whitespace()
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();

            let spec = match splitted.len() {
                4 => Spec {
                    channel: splitted.pop().unwrap(),
                    build: splitted.pop().unwrap(),
                    version: splitted.pop().unwrap(),
                    name: splitted.pop().unwrap(),
                },
                3 => Spec {
                    channel: "main".into(),
                    build: splitted.pop().unwrap(),
                    version: splitted.pop().unwrap(),
                    name: splitted.pop().unwrap(),
                },
                _ => return Err(Error::InvalidRecipe),
            };

            specs.push(spec);
        }

        Ok(Self { specs })
    }
}
