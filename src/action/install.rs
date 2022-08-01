use std::collections::{HashMap, VecDeque};

use indicatif::{ProgressBar, ProgressStyle};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    select, signal, spawn,
    sync::mpsc,
};

use super::{run_conda, spawn_conda};
use crate::recipe::{Package, Recipe, RecipeDiff};

pub async fn install(
    env_name: &str,
    new_recipe: &str,
    force_reinstall: bool,
    show_diff: bool,
) -> anyhow::Result<()> {
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
    let (old_recipe, need_create_env) = if old_recipe.is_none() || force_reinstall {
        ("".try_into().unwrap(), true)
    } else {
        (old_recipe.unwrap(), false)
    };
    let new_recipe: Recipe = Recipe::try_from(new_recipe).map_err(|e| anyhow::anyhow!(e))?;
    let channels = new_recipe.channels.clone();
    let diff = old_recipe.diff(new_recipe);
    if show_diff {
        println!("{:#}", diff);
    }

    let default_style = ProgressStyle::default_bar().template("{prefix:.bold.dim} {msg}");
    let pb = ProgressBar::new(1)
        .with_style(default_style.clone())
        .with_prefix("[1/3]")
        .with_message("checking env...");
    pb.tick();
    if need_create_env {
        pb.set_message(format!("creating env '{}'...", env_name));
        run_conda(["env", "remove", "-n", env_name]).await?;
        run_conda(["create", "-y", "--no-default-packages", "-n", env_name]).await?;
        pb.finish_with_message(format!("create env '{}' success", env_name));
    } else {
        pb.finish_with_message(format!("check env '{}' done", env_name));
    }

    let collections = collect_packages(&diff);

    // delete conda packages
    let delete_counts = collections.conda_delete_pkgs.len() + collections.pypi_delete_pkgs.len();
    let pb = ProgressBar::new(1)
        .with_style(default_style.clone())
        .with_prefix("[2/3]")
        .with_message(format!("deleting {} pkgs...", delete_counts));
    pb.tick();
    if !collections.conda_delete_pkgs.is_empty() {
        let mut args = vec!["remove", "-n", env_name, "--force", "-y"];
        let delete_pkg_names = collections
            .conda_delete_pkgs
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>();
        args.extend(delete_pkg_names);
        run_conda(args).await?;
    }
    // delete pypi packages
    if !collections.pypi_delete_pkgs.is_empty() {
        let mut args = vec!["run", "-n", env_name, "pip", "uninstall", "-y"];
        let delete_pkg_names = collections
            .pypi_delete_pkgs
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>();
        args.extend(delete_pkg_names);
        run_conda(args).await?;
    }
    pb.finish_with_message(format!("deleted {} pkgs", delete_counts));

    // install conda packages
    // spawn a printer
    let (event_tx, mut event_rx) = mpsc::channel::<InstallEvent>(10);
    let printer = spawn({
        let install_counts =
            collections.conda_install_pkgs.len() + collections.pypi_install_pkgs.len();

        let pb = if install_counts > 0 {
            ProgressBar::new(install_counts as u64)
                .with_style(
                    ProgressStyle::default_bar()
                        .template("{prefix:.bold.dim} {msg}\n{wide_bar} {pos}/{len}"),
                )
                .with_prefix("[3/3]")
                .with_message("installing pkgs...")
        } else {
            ProgressBar::new(install_counts as u64)
                .with_style(ProgressStyle::default_bar().template("{prefix:.bold.dim} {msg}"))
                .with_prefix("[3/3]")
                .with_message("installing pkgs...")
        };
        pb.tick();
        async move {
            loop {
                if let Some(event) = event_rx.recv().await {
                    match event {
                        InstallEvent::Message(s) => pb.println(s),
                        InstallEvent::Package(pkg) => pb.println(format!("installing {:#}", pkg)),
                        InstallEvent::Increase => pb.inc(1),
                        InstallEvent::Done => {
                            pb.finish_with_message(format!("installed {} pkgs", install_counts));
                            break;
                        }
                    }
                }
            }
        }
    });

    if !collections.conda_install_pkgs.is_empty() {
        let mut args = vec![
            "install",
            "--no-deps",
            "-S",
            "--force-reinstall",
            "-vv",
            "-y",
            "-n",
            env_name,
        ];
        let channels = channels
            .iter()
            .map(|c| ["-c", c])
            .collect::<Vec<_>>()
            .concat();
        args.extend(channels);
        let pkgs = collections
            .conda_install_pkgs
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>();
        let pkgs = pkgs.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        args.extend(pkgs);
        let mut child = spawn_conda(args)?;
        let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();

        // indexes are used to map id from conda log to pkg
        let indexes = collections
            .conda_install_pkgs
            .iter()
            .map(|p| {
                let id = match &p.kind {
                    crate::recipe::PackageKind::PyPi => format!("{}-{}", p.name, p.version),
                    crate::recipe::PackageKind::Conda { build, channel: _ } => {
                        format!("{}-{}-{}", p.name, p.version, build)
                    }
                };
                (id, p.clone())
            })
            .collect::<HashMap<_, _>>();
        let pattern = regex::Regex::new("==> LINKING PACKAGE: (?:.*?)::(.*) <==")?;
        let mut first_pkg = false;

        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        let _ = event_tx
            .send(InstallEvent::Message(
                "verifying environment...".to_string(),
            ))
            .await;
        loop {
            select! {
                stdout_line = stdout.next_line() => {
                    if let Ok(Some(line)) = stdout_line {
                        if line.starts_with("Verifying transaction: done") {
                            let _ = event_tx.send(InstallEvent::Message("verifying environment done".to_string())).await;
                        }
                    }
                },
                stderr_line = stderr.next_line() => {
                    if let Ok(Some(line)) = stderr_line {
                        if let Some(cap) = pattern.captures(&line) {
                            if !first_pkg {
                                first_pkg = false;
                                let _ = event_tx.send(InstallEvent::Increase).await;
                            }
                            let pkg = cap.get(1).unwrap().as_str().to_string();
                            let pkg = indexes[&pkg].clone();
                            let _ = event_tx.send(InstallEvent::Package(pkg)).await;
                        }
                    }
                },
                _ = sigterm.recv() => {
                    child.kill().await?;
                    return Err(anyhow::anyhow!("receive sigterm"));
                }
                _ = signal::ctrl_c() => {
                    child.kill().await?;
                    return Err(anyhow::anyhow!("receive ctrl c"));
                }
                _ = child.wait() => {
                    break
                }
            }
        }
    }
    // install pypi packages
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
        let max_failed = 50;
        let mut current_failed = 0;
        while !pkgs.is_empty() {
            let pkg = pkgs.pop_front().unwrap();
            let _ = event_tx.send(InstallEvent::Package(pkg.clone())).await;
            match run_conda([
                "run",
                "-n",
                env_name,
                "pip",
                "install",
                "--no-deps",
                pkg.to_string().as_str(),
            ])
            .await
            {
                Ok(_) => {
                    let _ = event_tx.send(InstallEvent::Increase).await;
                }
                Err(err) => {
                    if err.to_string().contains("not find a version") {
                        return Err(err);
                    } else {
                        current_failed += 1;
                        if current_failed == max_failed {
                            return Err(err);
                        }
                        // push current pkg back to pkgs
                        pkgs.push_back(pkg);
                        let _ = event_tx.send(InstallEvent::Message(format!(
                            "fail to install {:#}, will try to install it later\n{}",
                            pkg, err
                        )));
                    }
                }
            }
        }
    }

    let _ = event_tx.send(InstallEvent::Done).await;
    let _ = printer.await;

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

enum InstallEvent {
    Message(String),
    Package(Package),
    Increase,
    Done,
}

#[tokio::test]
async fn t() -> anyhow::Result<()> {
    install(
        "demo",
        r#"
# Name                    Version                   Build  Channel
ca-certificates           2022.07.19           hecd8cb5_0
certifi                   2022.6.15        py37hecd8cb5_0
libcxx                    12.0.0               h2f01273_0
libffi                    3.3                  hb1e8313_2
ncurses                   6.3                  hca72f7f_3
openssl                   1.1.1q               hca72f7f_0
pip                       22.1.2           py37hecd8cb5_0
python                    3.7.13               hdfd78df_0
readline                  8.1.2                hca72f7f_1
setuptools                61.2.0           py37hecd8cb5_0
sqlite                    3.38.5               h707629a_0
tk                        8.6.12               h5d9f67b_0
wheel                     0.37.1             pyhd3eb1b0_0
xz                        5.2.5                hca72f7f_1
zlib                      1.2.12               h4dc903c_2
django                    3.2.14                   pypi_0    pypi
"#,
        false,
        true,
    )
    .await?;

    Ok(())
}
