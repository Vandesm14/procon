pub mod config;
pub mod instance;
pub mod multi;

use std::{
  path::PathBuf,
  process::{Command, Stdio},
  str::FromStr,
  sync::LazyLock,
};

pub static SELF_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
  // Absolute path to the currently running executable.
  // On Linux this resolves /proc/self/exe to a real path.
  let exe = std::env::current_exe().expect("cannot get current exe");
  // If it's a symlink, canonicalize to the real file (best-effort).
  exe.canonicalize().unwrap_or(exe)
});

pub static NIX_SHELL_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
  PathBuf::from_str("/nix/var/nix/profiles/default/bin/nix-shell").unwrap()
});

pub fn nix_shell<'a, T>(
  path: &PathBuf,
  deps: Option<T>,
  cmds: &[String],
  inherit: bool,
) -> Command
where
  T: Iterator<Item = &'a String>,
{
  if let Some(deps) = deps {
    let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
    if inherit {
      cmd.stdout(Stdio::inherit());
      cmd.stderr(Stdio::inherit());
      cmd.stdin(Stdio::inherit());
    }

    cmd.current_dir(path);
    cmd
      .arg("-p")
      .args(deps)
      .arg("--run")
      .arg(cmds.to_vec().join("&&"));
    cmd
  } else {
    let mut cmd = Command::new("/usr/bin/env");
    cmd.arg("bash");

    if inherit {
      cmd.stdout(Stdio::inherit());
      cmd.stderr(Stdio::inherit());
      cmd.stdin(Stdio::inherit());
    }

    cmd.current_dir(path);
    cmd.arg("-c").arg(cmds.to_vec().join("&&"));
    cmd
  }
}
