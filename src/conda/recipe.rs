use crate::{Error, Result};

#[derive(Debug)]
pub struct CondaRecipe {
    specs: Vec<Spec>,
}

#[derive(Debug)]
struct Spec {
    name: String,
    version: String,
    build: String,
    channel: Option<String>,
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

            specs.push(spec);
        }

        Ok(Self { specs })
    }
}
