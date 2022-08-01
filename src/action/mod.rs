mod install;

pub use install::install;

use std::{ffi::OsStr, process::Stdio};

use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

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
