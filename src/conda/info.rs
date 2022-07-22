use std::process::Command;

use serde::Deserialize;

use crate::Result;

#[derive(Debug)]
pub struct CondaInfo {
    root_prefix: String,
    channel_alias: String,
    default_channels: Vec<String>,
    subdirs: Vec<String>,
}

impl CondaInfo {
    pub fn try_new(conda_bin: &str) -> Result<Self> {
        CondaConfig::try_new(conda_bin).map(|c| c.into())
    }
}

impl From<CondaConfig> for CondaInfo {
    fn from(config: CondaConfig) -> Self {
        Self {
            root_prefix: config.root_prefix,
            channel_alias: format!(
                "{}://{}",
                config.channel_alias.scheme, config.channel_alias.location
            ),
            default_channels: config
                .default_channels
                .into_iter()
                .map(|v| v.name.unwrap())
                .collect(),
            subdirs: config.subdirs,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CondaConfig {
    root_prefix: String,
    channel_alias: ChannelInfo,
    default_channels: Vec<ChannelInfo>,
    subdirs: Vec<String>,
}

impl CondaConfig {
    fn try_new(conda_bin: &str) -> Result<Self> {
        let ret = Command::new(conda_bin)
            .args(["config", "--show", "--json"])
            .output()?;

        Ok(serde_json::from_slice::<CondaConfig>(&ret.stdout)?)
    }
}

#[derive(Debug, Deserialize)]
struct ChannelInfo {
    name: Option<String>,
    scheme: String,
    location: String,
}
