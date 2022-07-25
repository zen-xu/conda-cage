use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueHint};

use cargo_cage::Result;

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
    version: Option<String>,
    file: Option<PathBuf>,
    conda_bin: Option<PathBuf>,
    force: bool,
    rename: Option<String>,
) -> Result<()> {
    Ok(())
}

fn validate_path(path: &str) -> std::result::Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("No such file or directory".to_string());
    }

    Ok(path)
}
