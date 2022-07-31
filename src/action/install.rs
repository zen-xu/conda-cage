use std::{collections::VecDeque, ffi::OsStr, process::Stdio};

use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

use crate::recipe::{Package, Recipe, RecipeDiff};

async fn install(
    env_name: &str,
    new_recipe: &str,
    force_reinstall: bool,
    show_diff: bool,
) -> anyhow::Result<()> {
    // Step 1: check env existence, if not exist, create it
    let old_recipe = {
        match run_conda(["list", "-n", env_name]).await {
            Ok(contents) => {
                Some(Recipe::try_from(contents.as_str()).map_err(|e| anyhow::anyhow!(e))?)
            }
            Err(error) => {
                if error.to_string().contains("EnvironmentLocationNotFound") {
                    None
                } else {
                    // get env recipe failed
                    return Err(error);
                }
            }
        }
    };
    let old_recipe = if old_recipe.is_none() || force_reinstall {
        run_conda(["env", "remove", "-n", env_name]).await?;
        run_conda(["create", "-y", "--no-default-packages", "-n", env_name]).await?;
        "".try_into().unwrap()
    } else {
        old_recipe.unwrap()
    };

    let new_recipe: Recipe = Recipe::try_from(new_recipe).map_err(|e| anyhow::anyhow!(e))?;
    let diff = old_recipe.diff(new_recipe);
    if show_diff {
        println!("{:#}", diff);
    }
    let collections = collect_packages(&diff);

    // Step 1: delete conda packages
    if !collections.conda_delete_pkgs.is_empty() {
        run_conda([
            "remove",
            "--force",
            "-y",
            "-n",
            env_name,
            &collections
                .conda_delete_pkgs
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        ])
        .await?;
    }
    // Step 2: delete pypi packages
    if !collections.pypi_delete_pkgs.is_empty() {
        let mut args = vec!["run", "-n", env_name, "pip", "uninstall", "-y"];
        let delete_pkg_names = collections
            .pypi_delete_pkgs
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>();
        args.extend(delete_pkg_names);
        run_conda(&args).await?;
    }
    // Step 3: install conda packages
    if !collections.conda_install_pkgs.is_empty() {
        run_conda([
            "install",
            "--no-deps",
            "-y",
            "-n",
            env_name,
            &collections
                .conda_install_pkgs
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        ])
        .await?;
    }
    // Step 4: install pypi packages
    if !collections.pypi_install_pkgs.is_empty() {
        if !collections
            .pypi_install_pkgs
            .iter()
            .filter(|p| p.name == "pip")
            .collect::<Vec<_>>()
            .is_empty()
        {
            // if need install `pip`, we should use conda install pip first, then use conda pip
            // upgrade pypi pip
            run_conda(["install", "--no-deps", "-y", "-n", env_name, "pip"]).await?;
        }

        let mut pkgs = VecDeque::from(collections.pypi_install_pkgs.clone());
        while !pkgs.is_empty() {
            let pkg = pkgs.pop_front().unwrap();
            run_conda([
                "run",
                "-n",
                env_name,
                "pip",
                "install",
                "--no-deps",
                pkg.to_string().as_str(),
            ])
            .await?;
        }
    }

    Ok(())
}

fn collect_packages<'p>(diff: &'p RecipeDiff) -> CollectedPackages<'p> {
    let mut conda_install_pkgs = vec![];
    let mut conda_delete_pkgs = vec![];
    let mut pypi_install_pkgs = vec![];
    let mut pypi_delete_pkgs = vec![];

    for pkg in &diff.adds {
        match &pkg.kind {
            crate::recipe::PackageKind::PyPi => pypi_install_pkgs.push(pkg),
            crate::recipe::PackageKind::Conda {
                build: _,
                channel: _,
            } => conda_install_pkgs.push(pkg),
        }
    }

    for update in &diff.updates {
        match (&update.from.kind, &update.to.kind) {
            (crate::recipe::PackageKind::PyPi, crate::recipe::PackageKind::PyPi) => {
                pypi_install_pkgs.push(&update.to)
            }
            (
                crate::recipe::PackageKind::PyPi,
                crate::recipe::PackageKind::Conda {
                    build: _,
                    channel: _,
                },
            ) => {
                pypi_delete_pkgs.push(&update.from);
                conda_install_pkgs.push(&update.to);
            }
            (
                crate::recipe::PackageKind::Conda {
                    build: _,
                    channel: _,
                },
                crate::recipe::PackageKind::PyPi,
            ) => {
                conda_delete_pkgs.push(&update.from);
                pypi_install_pkgs.push(&update.to);
            }
            (
                crate::recipe::PackageKind::Conda {
                    build: _,
                    channel: _,
                },
                crate::recipe::PackageKind::Conda {
                    build: _,
                    channel: _,
                },
            ) => {
                conda_delete_pkgs.push(&update.from);
                conda_install_pkgs.push(&update.to);
            }
        }
    }

    for pkg in &diff.deletes {
        match pkg.kind {
            crate::recipe::PackageKind::PyPi => pypi_delete_pkgs.push(pkg),
            crate::recipe::PackageKind::Conda {
                build: _,
                channel: _,
            } => conda_delete_pkgs.push(pkg),
        }
    }

    // sort pypi pkgs
    pypi_install_pkgs.sort_by(|a, b| match (a.name.as_str(), b.name.as_str()) {
        ("pip" | "wheel" | "setuptools" | "six", _) => std::cmp::Ordering::Less,
        (_, "pip" | "wheel" | "setuptools" | "six") => std::cmp::Ordering::Less,
        (_, _) => a.name.cmp(&b.name),
    });

    CollectedPackages {
        conda_install_pkgs,
        conda_delete_pkgs,
        pypi_install_pkgs,
        pypi_delete_pkgs,
    }
}

#[derive(Debug)]
struct CollectedPackages<'p> {
    conda_install_pkgs: Vec<&'p Package>,
    conda_delete_pkgs: Vec<&'p Package>,
    pypi_install_pkgs: Vec<&'p Package>,
    pypi_delete_pkgs: Vec<&'p Package>,
}

/// this function will not block and return Child
fn spawn_conda<I, S>(args: I) -> std::io::Result<Child>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("conda")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// this function will block and return stdout when success
async fn run_conda<I, S>(args: I) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut process = spawn_conda(args)?;
    let mut msg = String::new();
    if process.wait().await.unwrap().success() {
        let mut stdout = process.stdout.unwrap();
        let _ = stdout.read_to_string(&mut msg).await;
        Ok(msg)
    } else {
        let mut stderr = process.stderr.unwrap();
        let _ = stderr.read_to_string(&mut msg).await;
        Err(anyhow::anyhow!(msg))
    }
}

#[tokio::test]
async fn t() -> anyhow::Result<()> {
    install(
        "demo",
        r#"
# Name                    Version                   Build  Channel
bzip2                     1.0.8                h620ffc9_4
ca-certificates           2022.07.19           hca03da5_0
certifi                   2022.6.15       py310hca03da5_0
libcxx                    12.0.0               hf6beb65_1
libffi                    3.4.2                hc377ac9_4
ncurses                   6.3                  h1a28f6b_3
openssl                   1.1.1q               h1a28f6b_0
pip                       22.1.2          py310hca03da5_0
python                    3.10.4               hbdb9e5c_0
readline                  8.1.2                h1a28f6b_1
setuptools                61.2.0          py310hca03da5_0
sqlite                    3.38.5               h1058600_0
tk                        8.6.12               hb8d0fd4_0
tzdata                    2022a                hda174b7_0
wheel                     0.37.1             pyhd3eb1b0_0
xz                        5.2.5                h1a28f6b_1
werkzeug                  2.2.1                    pypi_0    pypi
zlib                      1.2.12               h5a0b063_2
django                    4.0.6                    pypi_0    pypi
"#,
        false,
        true,
    )
    .await?;

    Ok(())
}
