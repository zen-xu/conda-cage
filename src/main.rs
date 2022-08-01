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
            help = "Install the env by the local given file"
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
            help = "Show the difference between local env and target env"
        )]
        show_diff: bool,

        #[clap(long, value_parser, help = "Rename the installing env name")]
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
            let new_recipe = if let Some(file) = file {
                std::fs::read_to_string(file)?
            } else {
                let version = version
                    .or(Some("master".to_string()))
                    .map(|v| {
                        if v == "latest" {
                            "master".to_string()
                        } else {
                            v
                        }
                    })
                    .unwrap();
                fetch_recipe(&env_name, &version).await?
            };
            let env_name = rename.unwrap_or(env_name);
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

async fn fetch_recipe(env_name: &str, version: &str) -> anyhow::Result<String> {
    let rsp = reqwest::get(format!(
        "http://hftgitlab/conda-envs/{}/raw/{}/env.recipe?inline=false",
        env_name, version
    ))
    .await?;
    if !rsp.status().is_success() {
        return Err(anyhow::anyhow!(
            "fail to fetch env: {}, version: {}, err code: {}",
            env_name,
            version,
            rsp.status()
        ));
    }
    Ok(rsp.text().await?)
}
