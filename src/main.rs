use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::read_link,
    io::Read,
    os::unix::fs::symlink,
    path::PathBuf,
    process::{self, Command, Stdio},
};

use anyhow::anyhow;
use cargo_cage::{
    conda::{cache::PathType, recipe::Spec, CondaCache, CondaRecipe},
    CondaIndex, CondaInfo,
};
use clap::{Parser, Subcommand, ValueHint};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[clap(about = "Install conda conda")]
    Install {
        #[clap(value_parser, help = "The env name you need to install")]
        env_name: String,

        #[clap(long, value_parser, help = "Specify the version of env")]
        version: Option<String>,

        #[clap(
            short,
            long,
            value_hint = ValueHint::FilePath,
            value_parser = validate_path,
            help = "Install the env from the given file"
        )]
        file: Option<PathBuf>,

        #[clap(
            long,
            help = "Specify the conda bin path",
            value_parser = validate_path,
            value_hint = ValueHint::FilePath
        )]
        conda_bin: Option<PathBuf>,

        #[clap(
            long,
            action,
            help = "Force to install the env, and this will try to remove the local env first"
        )]
        force: bool,

        #[clap(long, value_parser, help = "Rename the install env name")]
        rename: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Install {
            env_name,
            version,
            file,
            conda_bin,
            force,
            rename,
        } => {
            install(env_name, version, file, conda_bin, force, rename)?;
        }
    }

    Ok(())
}

fn install(
    env_name: String,
    _version: Option<String>,
    file: Option<PathBuf>,
    conda_bin: Option<PathBuf>,
    force: bool,
    _rename: Option<String>,
) -> anyhow::Result<()> {
    let conda_bin = if let Some(conda_bin) = conda_bin {
        conda_bin.display().to_string()
    } else {
        "conda".to_string()
    };

    let conda_info = CondaInfo::try_new(&conda_bin)?;
    let conda = Conda::new(&conda_bin);
    let conda_cache = CondaCache::new(&conda_info);

    if force {
        conda.execute(["env", "remove", "-n", &env_name])?;
    }

    let old_recipe = match conda.get_env_recipe(&env_name) {
        Some(recipe) => recipe,
        None => CondaRecipe::try_new("")?,
    };
    let new_recipe = if let Some(file) = file {
        let data = std::fs::read_to_string(file)?;
        CondaRecipe::try_new(&data)?
    } else {
        CondaRecipe::try_new("")?
    };
    let diff = old_recipe.diff(&new_recipe);
    let default_style = ProgressStyle::default_bar().template("{prefix:.bold.dim} {msg}");

    // Step 1: verify conda indexes
    let pb = ProgressBar::new(1)
        .with_style(default_style.clone())
        .with_prefix("[1/5]")
        .with_message("verifying conda indexes...");
    pb.tick();

    let mut channels = diff
        .conda
        .add
        .iter()
        .filter(|v| v.channel.is_some())
        .map(|v| v.channel.clone().unwrap())
        .collect::<HashSet<String>>();
    // extend default channels
    channels.extend(conda_info.default_channels.clone());
    let channels = channels.into_iter().collect::<Vec<String>>();
    let mut conda_index = CondaIndex::try_new(&conda_info, &conda_cache, &channels)?;
    pb.finish_with_message("verify conda indexes done");

    // Step 2: check whether env exists
    let pb = ProgressBar::new(1)
        .with_style(default_style.clone())
        .with_prefix("[2/5]")
        .with_message("checking env..");
    pb.tick();
    if !conda.check_env_exists(&env_name) {
        pb.set_message(format!("not found env {}, creating it...", env_name));
        let _ = conda.execute(["create", "-n", &env_name, "--no-default-packages"]);
        pb.finish_with_message(format!("create env '{}' success", env_name));
    } else {
        pb.finish_with_message("check env done");
    }

    let env_root_dir = PathBuf::from(&conda_info.root_prefix)
        .join("envs")
        .join(env_name);

    // Step 3: install new pkgs
    let total = (diff.conda.add.len() + diff.pypi.add.len()) as u64;
    let pb = ProgressBar::new(total)
        .with_style(
            ProgressStyle::default_bar()
                .template("{prefix:.bold.dim} {msg}\n{wide_bar} {pos}/{len}"),
        )
        .with_prefix("[3/5]")
        .with_message("installing new pkgs..");
    // install conda pkgs first
    for spec in diff.conda.add {
        pb.println(format!(
            "installing {}:{}:{}...",
            spec.name, spec.version, spec.build
        ));
        install_conda_pkg(
            &env_root_dir,
            &mut conda_index,
            &conda_cache,
            &spec,
            &channels,
        )?;
        pb.inc(1);
    }
    // then install pypi pkgs
    for spec in diff.pypi.add {
        pb.println(format!("installing {}:{}...", spec.name, spec.version));
        install_pypi_pkg(&env_root_dir, &spec)?;
        pb.println(format!("installed {}:{}...", spec.name, spec.version));
        pb.inc(1);
    }
    pb.finish_with_message(format!("added {} new pkgs", total));

    // Step 4: update pkgs
    let total = (diff.conda.update.len() + diff.pypi.update.len()) as u64;
    let pb = ProgressBar::new(total)
        .with_style(
            ProgressStyle::default_bar()
                .template("{prefix:.bold.dim} {msg}\n{wide_bar} {pos}/{len}"),
        )
        .with_prefix("[4/5]")
        .with_message("updating pkgs..");
    for update in diff.conda.update {
        pb.println(format!(
            "updating {}:{}:{} => {}:{}:{}...",
            update.from.name,
            update.from.version,
            update.from.build,
            update.to.name,
            update.to.version,
            update.to.build,
        ));
        uninstall_conda_pkg(&env_root_dir, &conda_index, &conda_cache, &update.from)?;
        install_conda_pkg(
            &env_root_dir,
            &mut conda_index,
            &conda_cache,
            &update.to,
            &channels,
        )?;
        pb.inc(1);
    }
    for update in diff.pypi.update {
        pb.println(format!(
            "updating {}:{} => {}:{}...",
            update.from.name, update.from.version, update.to.name, update.to.version,
        ));
        uninstall_pypi_pkg(&env_root_dir, &update.from)?;
        install_pypi_pkg(&env_root_dir, &update.to)?;
        pb.inc(1);
    }
    pb.finish_with_message(format!("updated {} pkgs", total));

    // Step 5: delete pkgs
    let total = (diff.conda.delete.len() + diff.pypi.delete.len()) as u64;
    let pb = ProgressBar::new(total)
        .with_style(
            ProgressStyle::default_bar()
                .template("{prefix:.bold.dim} {msg}\n{wide_bar} {pos}/{len}"),
        )
        .with_prefix("[5/5]")
        .with_message("deleting pkgs..");
    for spec in &diff.conda.delete {
        pb.set_message(format!(
            "deleting {}:{}:{}...",
            spec.name, spec.version, spec.build,
        ));
        uninstall_conda_pkg(&env_root_dir, &conda_index, &conda_cache, spec)?;
        pb.inc(1);
    }
    for spec in &diff.conda.delete {
        pb.set_message(format!("deleting {}:{}...", spec.name, spec.version));
        uninstall_pypi_pkg(&env_root_dir, spec)?;
        pb.inc(1);
    }
    pb.finish_with_message(format!("deleted {} pkgs", total));

    Ok(())
}

fn validate_path(path: &str) -> std::result::Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("No such file or directory".to_string());
    }

    Ok(path)
}

#[derive(Clone)]
struct Conda {
    bin: String,
}

impl Conda {
    fn new(conda_bin: &str) -> Self {
        Self {
            bin: conda_bin.to_string(),
        }
    }

    fn execute<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(
        &self,
        args: I,
    ) -> std::io::Result<process::Output> {
        Command::new(&self.bin).args(args).output()
    }

    fn get_env_recipe(&self, env_name: &str) -> Option<CondaRecipe> {
        if let Ok(out) = self.execute(["list", "-n", env_name]) {
            if out.status.success() {
                let recipe = String::from_utf8(out.stdout).unwrap();
                return Some(CondaRecipe::try_new(&recipe).unwrap());
            }
        }

        None
    }

    fn check_env_exists(&self, env_name: &str) -> bool {
        self.get_env_recipe(env_name).is_some()
    }
}

fn install_conda_pkg(
    env_root_dir: &PathBuf,
    conda_index: &mut CondaIndex,
    conda_cache: &CondaCache,
    spec: &Spec,
    channels: &Vec<String>,
) -> anyhow::Result<()> {
    let pkg = conda_index.get_by_spec(&spec);
    let pkg = if let Some(pkg) = pkg {
        pkg
    } else {
        // update index
        conda_index.update_indexes(channels)?;
        conda_index.get_by_spec(&spec).ok_or_else(|| {
            anyhow!(
                "not found {}:{}:{} in indexes",
                spec.name,
                spec.version,
                spec.build
            )
        })?
    };

    if conda_cache.get_tarball(&pkg).is_none() {
        conda_index.download(&pkg)?;
    }
    let extracted_dir = conda_cache.get_extracted_dir(&pkg).unwrap();
    let prefix_record = conda_cache.try_get_prefix_record(&pkg)?;
    let cwd = std::env::current_dir()?;

    let python_bin = env_root_dir.join("bin/python").display().to_string();
    for file in prefix_record.paths() {
        let from = &extracted_dir.join(&file.path);
        let to = &env_root_dir.join(&file.path);
        if !to.parent().unwrap().exists() {
            std::fs::create_dir_all(to.parent().unwrap())?;
        }
        std::env::set_current_dir(to.parent().unwrap())?;

        match file.path_type {
            PathType::HardLink => {
                if to.exists() {
                    std::fs::remove_file(to)?;
                }
                std::fs::hard_link(from, to)?;
                if let Some(prefix) = &file.prefix_placeholder {
                    let contents = std::fs::read_to_string(to)?;
                    let contents = contents.replace(prefix, &format!("#!{}", python_bin));
                    std::fs::write(to, contents)?;
                }
            }
            PathType::SoftLink => {
                let original = read_link(from)?;
                let link = PathBuf::from(from.file_name().unwrap());
                if link.exists() {
                    std::fs::remove_file(&link)?;
                }
                symlink(&original, &link)?;
            }
            PathType::Directory => {
                std::fs::create_dir_all(to)?;
            }
        }
    }
    std::env::set_current_dir(cwd)?;
    // dump prefix record
    let conda_meta_dir = env_root_dir.join("conda-meta");
    if !conda_meta_dir.exists() {
        std::fs::create_dir_all(&conda_meta_dir)?;
    }
    std::fs::write(
        conda_meta_dir.join(format!(
            "{}-{}-{}.json",
            spec.name, spec.version, spec.build
        )),
        serde_json::to_string(&prefix_record)?,
    )?;

    Ok(())
}

fn uninstall_conda_pkg(
    env_root_dir: &PathBuf,
    conda_index: &CondaIndex,
    conda_cache: &CondaCache,
    spec: &Spec,
) -> anyhow::Result<()> {
    let pkg = conda_index.get_by_spec(spec).unwrap();
    let prefix_record = conda_cache.try_get_prefix_record(&pkg)?;
    for file in prefix_record.paths() {
        let to = &env_root_dir.join(&file.path);
        match file.path_type {
            PathType::Directory => {
                // skip
            }
            _ => {
                std::fs::remove_file(to)?;
            }
        }
    }
    Ok(())
}

fn install_pypi_pkg(env_root_dir: &PathBuf, spec: &Spec) -> anyhow::Result<()> {
    //panic!("{}", env_root_dir.join("bin").join("pip").display());
    let pip_path = env_root_dir.join("bin").join("pip").display().to_string();
    let process = Command::new(&pip_path)
        .args([
            "install",
            "--no-deps",
            &format!("{}=={}", spec.name, spec.version),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut s = String::new();
    match process.stderr.unwrap().read_to_string(&mut s) {
        Err(e) => Err(e)?,
        Ok(_) => Err(anyhow!(s))?,
    }
}

fn uninstall_pypi_pkg(env_root_dir: &PathBuf, spec: &Spec) -> anyhow::Result<()> {
    let process = Command::new(env_root_dir.join("bin/pip"))
        .args(["uninstall", "-y", &spec.name])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut s = String::new();
    match process.stderr.unwrap().read_to_string(&mut s) {
        Err(e) => Err(e)?,
        Ok(_) => Err(anyhow!(s))?,
    }
}

#[test]
fn run() -> anyhow::Result<()> {
    let process = Command::new("ls")
        .args(["."])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut s = String::new();
    match process.stderr.unwrap().read_to_string(&mut s) {
        Err(why) => Err(why)?,
        Ok(_) => println!("ffff {}", s),
    }
    Ok(())
}

#[test]
fn get_symlink() {
    symlink("123", "123.bk");
}
