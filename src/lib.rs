pub mod config;
pub mod instance;

use std::{
  path::PathBuf,
  process::{Command, Output, Stdio},
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

pub fn nix_shell(
  path: PathBuf,
  deps: Vec<String>,
  cmds: Vec<String>,
  piped: bool,
) -> std::io::Result<Output> {
  let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
  if piped {
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
  }

  cmd.current_dir(path);
  cmd
    .arg("-p")
    .args(deps)
    .arg("--run")
    .arg(cmds.to_vec().join("&&"));
  cmd.output()
}
