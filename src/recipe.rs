use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use console::style;

#[derive(Debug, PartialEq)]
pub struct Recipe {
    pub channels: HashSet<String>,
    pub packages: HashMap<String, Package>,
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
pub enum PackageKind {
    PyPi,
    Conda { build: String, channel: String },
}

impl TryFrom<&str> for Recipe {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut packages = HashMap::new();
        let mut channels = HashSet::new();
        for line in value.lines() {
            let line = line.trim();
            if line.starts_with("#") || line.is_empty() {
                continue;
            }

            let splitted = line.split_whitespace().into_iter().collect::<Vec<_>>();
            let package = match splitted[..] {
                [name, version, build] => {
                    // conda package
                    let channel = "defaults".to_string();
                    channels.insert(channel.clone());
                    Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        kind: PackageKind::Conda {
                            build: build.to_string(),
                            channel,
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
                    channels.insert(channel.to_string());
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
            channels: HashSet::from(["conda-forge".into(), "defaults".into()]),
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
                            channel: "defaults".into()
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

#[derive(Debug, Default, PartialEq)]
pub struct RecipeDiff {
    pub adds: Vec<Package>,
    pub updates: Vec<Update>,
    pub deletes: Vec<Package>,
}

impl RecipeDiff {
    fn sort(&mut self) {
        self.adds.sort_by(|a, b| a.name.cmp(&b.name));
        self.updates.sort_by(|a, b| a.from.name.cmp(&b.from.name));
        self.deletes.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

impl Display for RecipeDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.adds.is_empty() {
            writeln!(
                f,
                "{}",
                style(format!("Add {} packages:", self.adds.len()))
                    .green()
                    .bold()
            )?;
            for pkg in &self.adds {
                writeln!(f, " {} {:#}", style("+").green().to_string(), pkg)?;
            }
        }

        if !self.updates.is_empty() {
            writeln!(
                f,
                "{}",
                style(format!("Update {} packages:", self.updates.len()))
                    .blue()
                    .bold()
            )?;
            for Update { from, to } in &self.updates {
                writeln!(
                    f,
                    " {} {:#} => {:#}",
                    style("*").blue().to_string(),
                    from,
                    to
                )?;
            }
        }

        if !self.deletes.is_empty() {
            writeln!(
                f,
                "{}",
                style(format!("Delete {} packages:", self.deletes.len()))
                    .red()
                    .bold()
            )?;
            for pkg in &self.deletes {
                writeln!(f, " {} {:#}", style("-").red().to_string(), pkg)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Update {
    pub from: Package,
    pub to: Package,
}

impl Recipe {
    pub fn diff(self, mut new_recipe: Self) -> RecipeDiff {
        let mut diff = RecipeDiff::default();
        for (pkg_name, old_pkg) in self.packages {
            if let Some(new_pkg) = new_recipe.packages.remove(&pkg_name) {
                if new_pkg != old_pkg {
                    diff.updates.push(Update {
                        from: old_pkg,
                        to: new_pkg,
                    })
                }
            } else {
                diff.deletes.push(old_pkg)
            }
        }
        diff.adds = new_recipe.packages.into_iter().map(|(_, v)| v).collect();

        // sort
        diff.sort();
        diff
    }
}

#[test]
fn diff_two_recipe() {
    use PackageKind::{Conda, PyPi};

    let old_recipe: Recipe = r#"
# Name                    Version                   Build  Channel
aiohttp                   3.8.1                    pypi_0    pypi
blas                      1.0                         mkl
certifi                   2022.6.15        py37hecd8cb5_0    conda-forge
frozenlist                1.3.0                    pypi_0    pypi
multidict                 6.0.2                    pypi_0    pypi
ncurses                   6.3                  hca72f7f_3
numpy                     1.18.1           py37h7241aed_0
yarl                      1.7.2                    pypi_0    pypi
"#
    .try_into()
    .unwrap();
    let new_recipe: Recipe = r#"
# Name                    Version                   Build  Channel
aiohttp                   3.8.2                    pypi_0    pypi
blas                      1.2                         mkl
idna                      3.3                      pypi_0    pypi
libcxx                    12.0.0               h2f01273_0
multidict                 6.0.2                    pypi_0    pypi
ncurses                   6.3                  hca72f7f_3
numpy                     1.18.2                   pypi_0    pypi
yarl                      1.7.3                xaa72f7f_3    conda-forge
"#
    .try_into()
    .unwrap();

    let diff = old_recipe.diff(new_recipe);
    let mut expected = RecipeDiff {
        adds: vec![
            Package {
                name: "idna".into(),
                version: "3.3".into(),
                kind: PyPi,
            },
            Package {
                name: "libcxx".into(),
                version: "12.0.0".into(),
                kind: Conda {
                    build: "h2f01273_0".into(),
                    channel: "defaults".into(),
                },
            },
        ],
        updates: vec![
            Update {
                from: Package {
                    name: "numpy".into(),
                    version: "1.18.1".into(),
                    kind: Conda {
                        build: "py37h7241aed_0".into(),
                        channel: "defaults".into(),
                    },
                },
                to: Package {
                    name: "numpy".into(),
                    version: "1.18.2".into(),
                    kind: PyPi,
                },
            },
            Update {
                from: Package {
                    name: "blas".into(),
                    version: "1.0".into(),
                    kind: Conda {
                        build: "mkl".into(),
                        channel: "defaults".into(),
                    },
                },
                to: Package {
                    name: "blas".into(),
                    version: "1.2".into(),
                    kind: Conda {
                        build: "mkl".into(),
                        channel: "defaults".into(),
                    },
                },
            },
            Update {
                from: Package {
                    name: "yarl".into(),
                    version: "1.7.2".into(),
                    kind: PyPi,
                },
                to: Package {
                    name: "yarl".into(),
                    version: "1.7.3".into(),
                    kind: Conda {
                        build: "xaa72f7f_3".into(),
                        channel: "conda-forge".into(),
                    },
                },
            },
            Update {
                from: Package {
                    name: "aiohttp".into(),
                    version: "3.8.1".into(),
                    kind: PyPi,
                },
                to: Package {
                    name: "aiohttp".into(),
                    version: "3.8.2".into(),
                    kind: PyPi,
                },
            },
        ],
        deletes: vec![
            Package {
                name: "frozenlist".into(),
                version: "1.3.0".into(),
                kind: PyPi,
            },
            Package {
                name: "certifi".into(),
                version: "2022.6.15".into(),
                kind: Conda {
                    build: "py37hecd8cb5_0".into(),
                    channel: "conda-forge".into(),
                },
            },
        ],
    };
    expected.sort();
    assert_eq!(diff, expected);
}
