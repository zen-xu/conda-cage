use std::collections::HashMap;

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
            let (adds, deletes) = if old_spec.channel == Some("pypi".to_string()) {
                (&mut diff.pypi.add, &mut diff.pypi.delete)
            } else {
                (&mut diff.conda.add, &mut diff.conda.delete)
            };

            match new_recipe.specs.get(k) {
                Some(new_spec) => {
                    if new_spec != old_spec {
                        adds.push(new_spec.clone());
                        deletes.push(old_spec.clone());
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
struct Spec {
    name: String,
    version: String,
    build: String,
    channel: Option<String>,
}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version && self.build == other.build
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct DiffInfo {
    conda: Diff,
    pypi: Diff,
}

#[derive(Debug, Default, PartialEq)]
pub struct Diff {
    add: Vec<Spec>,
    delete: Vec<Spec>,
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

    assert_eq!(
        diff,
        DiffInfo {
            pypi: Diff {
                add: vec![],
                delete: vec![Spec {
                    name: "c".into(),
                    version: "0.3.0".into(),
                    build: "abc".into(),
                    channel: Some("pypi".into()),
                }]
            },
            conda: Diff {
                add: vec![
                    Spec {
                        name: "a".into(),
                        version: "0.2.0".into(),
                        build: "abc".into(),
                        channel: None,
                    },
                    Spec {
                        name: "d".into(),
                        version: "0.1.1".into(),
                        build: "ppp".into(),
                        channel: Some("conda-forge".into()),
                    }
                ],
                delete: vec![Spec {
                    name: "a".into(),
                    version: "0.1.0".into(),
                    build: "abc".into(),
                    channel: None,
                }],
            }
        }
    )
}
