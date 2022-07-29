use std::{collections::HashMap, fmt::Display};

use console::style;

#[derive(Debug, PartialEq)]
pub struct Recipe {
    pub channels: Vec<String>,
    pub packages: HashMap<String, Package>,
}

#[derive(Debug, PartialEq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub kind: PackageKind,
}

impl Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name_var = style("name").yellow().to_string();
        let version_var = style("version").yellow().to_string();
        let build_var = style("build").yellow().to_string();
        let channel_var = style("channel").yellow().to_string();
        match &self.kind {
            PackageKind::PyPi => {
                if f.alternate() {
                    write!(
                        f,
                        "{}{}{}={}, {}={}{}",
                        style("PyPi").magenta().dim().bold().to_string(),
                        style("(").white().dim().to_string(),
                        &name_var,
                        style(&self.name).cyan().dim().to_string(),
                        &version_var,
                        style(&self.version).cyan().dim().to_string(),
                        style(")").white().dim().to_string(),
                    )
                } else {
                    write!(f, "{}=={}", self.name, self.version)
                }
            }
            PackageKind::Conda { build, channel } => {
                if f.alternate() {
                    write!(
                        f,
                        "{}{}{}={}, {}={}, {}={}, {}={}{}",
                        style("Conda").magenta().dim().bold().to_string(),
                        style("(").white().dim().to_string(),
                        &name_var,
                        style(&self.name).cyan().dim().to_string(),
                        &version_var,
                        style(&self.version).cyan().dim().to_string(),
                        &build_var,
                        style(build).cyan().dim().to_string(),
                        &channel_var,
                        style(channel).cyan().dim().to_string(),
                        style(")").white().dim().to_string(),
                    )
                } else {
                    write!(f, "{}={}={}", self.name, self.version, build)
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PackageKind {
    PyPi,
    Conda { build: String, channel: String },
}

impl TryFrom<&str> for Recipe {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut packages = HashMap::new();
        let mut channels = vec![];
        for line in value.lines() {
            let line = line.trim();
            if line.starts_with("#") || line.is_empty() {
                continue;
            }

            let splitted = line.split_whitespace().into_iter().collect::<Vec<_>>();
            let package = match splitted[..] {
                [name, version, build] => {
                    // conda package
                    Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        kind: PackageKind::Conda {
                            build: build.to_string(),
                            channel: "default".to_string(),
                        },
                    }
                }
                [name, version, _, "pypi"] => {
                    // pypi package
                    Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        kind: PackageKind::PyPi,
                    }
                }
                [name, version, build, channel] => {
                    // conda other channel package
                    channels.push(channel.to_string());
                    Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        kind: PackageKind::Conda {
                            build: build.to_string(),
                            channel: channel.to_string(),
                        },
                    }
                }
                _ => {
                    return Err(format!("invalid package spec: {}", line));
                }
            };
            packages.insert(package.name.clone(), package);
        }

        Ok(Self { channels, packages })
    }
}

#[test]
fn test_serialize_recipe() {
    use PackageKind::{Conda, PyPi};

    let recipe_contents = r#"
# Name                    Version                   Build  Channel
aiohttp                   3.8.1                    pypi_0    pypi
blas                      1.0                         mkl
certifi                   2022.6.15        py37hecd8cb5_0    conda-forge
"#;
    let recipe: Recipe = recipe_contents.try_into().unwrap();
    assert_eq!(
        recipe,
        Recipe {
            channels: vec!["conda-forge".into()],
            packages: [
                (
                    "aiohttp",
                    Package {
                        name: "aiohttp".into(),
                        version: "3.8.1".into(),
                        kind: PyPi,
                    }
                ),
                (
                    "blas",
                    Package {
                        name: "blas".into(),
                        version: "1.0".into(),
                        kind: Conda {
                            build: "mkl".into(),
                            channel: "default".into()
                        }
                    }
                ),
                (
                    "certifi",
                    Package {
                        name: "certifi".into(),
                        version: "2022.6.15".into(),
                        kind: Conda {
                            build: "py37hecd8cb5_0".into(),
                            channel: "conda-forge".into()
                        }
                    }
                )
            ]
            .map(|(n, p)| (n.to_string(), p))
            .into()
        }
    )
}
