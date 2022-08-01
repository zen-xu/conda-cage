use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueHint};

use conda_cage::action;

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
            action,
            help = "Force to install the env, and this will try to remove the local env first"
        )]
        force: bool,

        #[clap(
            long,
            action,
            help = "Show the difference between local env and given env"
        )]
        show_diff: bool,

        #[clap(long, value_parser, help = "Rename the install env name")]
        rename: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Install {
            env_name,
            version,
            file,
            force,
            show_diff,
            rename,
        } => {
            let new_recipe = std::fs::read_to_string(file.unwrap())?;
            action::install(&env_name, &new_recipe, force, show_diff).await?;
        }
    }

    Ok(())
}

fn validate_path(path: &str) -> std::result::Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err("No such file or directory".to_string());
    }

    Ok(path)
}
