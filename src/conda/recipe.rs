use console::style;
use std::{collections::HashMap, fmt::Display};

use crate::{Error, Result};

#[derive(Debug)]
pub struct CondaRecipe {
    specs: HashMap<String, Spec>,
}

impl CondaRecipe {
    pub fn try_new(content: &str) -> Result<Self> {
        let mut specs = HashMap::new();

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
                    channel: Some(splitted.pop().unwrap()),
                    build: splitted.pop().unwrap(),
                    version: splitted.pop().unwrap(),
                    name: splitted.pop().unwrap(),
                },
                3 => Spec {
                    channel: None,
                    build: splitted.pop().unwrap(),
                    version: splitted.pop().unwrap(),
                    name: splitted.pop().unwrap(),
                },
                _ => return Err(Error::InvalidRecipe),
            };

            specs.insert(spec.name.clone(), spec);
        }

        Ok(Self { specs })
    }

    pub fn diff(&self, new_recipe: &CondaRecipe) -> DiffInfo {
        let mut diff = DiffInfo::default();
        for (k, old_spec) in &self.specs {
            let (updates, deletes) = if old_spec.channel == Some("pypi".to_string()) {
                (&mut diff.pypi.update, &mut diff.pypi.delete)
            } else {
                (&mut diff.conda.update, &mut diff.conda.delete)
            };

            match new_recipe.specs.get(k) {
                Some(new_spec) => {
                    if new_spec != old_spec {
                        updates.push(Update {
                            from: old_spec.clone(),
                            to: new_spec.clone(),
                        })
                    }
                }
                None => deletes.push(old_spec.clone()),
            }
        }

        for (_, new_spec) in &new_recipe.specs {
            let adds = if new_spec.channel == Some("pypi".to_string()) {
                &mut diff.pypi.add
            } else {
                &mut diff.conda.add
            };

            if !self.specs.contains_key(&new_spec.name) {
                adds.push(new_spec.clone());
            }
        }

        diff
    }
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub name: String,
    pub version: String,
    pub build: String,
    pub channel: Option<String>,
}

impl Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.name, self.version, self.build)
    }
}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version && self.build == other.build
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct DiffInfo {
    pub conda: Diff,
    pub pypi: Diff,
}

impl Display for DiffInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", style("Conda:").dim().bold())?;
        if !self.conda.add.is_empty() {
            writeln!(f, "  {}", style("Add:").green())?;
            for pkg in &self.conda.add {
                writeln!(f, "    {}", style(pkg.to_string()).yellow())?;
            }
        }
        if !self.conda.update.is_empty() {
            writeln!(f, "  {}", style("Update:").blue())?;
            for update in &self.conda.update {
                writeln!(
                    f,
                    "    {} => {}",
                    style(update.from.to_string()).yellow(),
                    style(update.to.to_string()).yellow()
                )?;
            }
        }
        if !self.conda.delete.is_empty() {
            writeln!(f, "  {}", style("Delete:").red())?;
            for pkg in &self.conda.delete {
                writeln!(f, "    {}", style(pkg.to_string()).yellow())?;
            }
        }

        writeln!(f, "{}", style("PyPi:").dim().bold())?;
        if !self.pypi.add.is_empty() {
            writeln!(f, "  {}", style("Add:").green())?;
            for pkg in &self.pypi.add {
                writeln!(f, "    {}", style(pkg.to_string()).yellow())?;
            }
        }
        if !self.pypi.update.is_empty() {
            writeln!(f, "  {}", style("Update:").blue())?;
            for update in &self.pypi.update {
                writeln!(
                    f,
                    "    {} => {}",
                    style(update.from.to_string()).yellow(),
                    style(update.to.to_string()).yellow()
                )?;
            }
        }
        if !self.pypi.delete.is_empty() {
            writeln!(f, "  {}", style("Delete:").red())?;
            for pkg in &self.pypi.delete {
                writeln!(f, "    {}", style(pkg.to_string()).yellow())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Diff {
    pub add: Vec<Spec>,
    pub update: Vec<Update>,
    pub delete: Vec<Spec>,
}

#[derive(Debug, PartialEq)]
pub struct Update {
    pub from: Spec,
    pub to: Spec,
}

#[test]
fn test_diff() {
    let old_recipe = r#"
a 0.1.0 abc
b 0.2.0 ddd
c 0.3.0 abc pypi
"#;
    let new_recipe = r#"
a 0.2.0 abc
b 0.2.0 ddd
d 0.1.1 ppp conda-forge
"#;
    let old = CondaRecipe::try_new(old_recipe).unwrap();
    let new = CondaRecipe::try_new(new_recipe).unwrap();
    let diff = old.diff(&new);
    println!("{}", diff);

    assert_eq!(
        diff,
        DiffInfo {
            pypi: Diff {
                add: vec![],
                update: vec![],
                delete: vec![Spec {
                    name: "c".into(),
                    version: "0.3.0".into(),
                    build: "abc".into(),
                    channel: Some("pypi".into()),
                }]
            },
            conda: Diff {
                add: vec![Spec {
                    name: "d".into(),
                    version: "0.1.1".into(),
                    build: "ppp".into(),
                    channel: Some("conda-forge".into()),
                }],
                update: vec![Update {
                    from: Spec {
                        name: "a".into(),
                        version: "0.1.0".into(),
                        build: "abc".into(),
                        channel: None,
                    },
                    to: Spec {
                        name: "a".into(),
                        version: "0.2.0".into(),
                        build: "abc".into(),
                        channel: None,
                    },
                }],
                delete: vec![],
            }
        }
    )
}
