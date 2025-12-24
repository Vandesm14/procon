pub mod config;
pub mod instance;
pub mod multi;

use std::{
  path::{Path, PathBuf},
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

fn escape_bash_string(s: &str) -> String {
  // Escape single quotes by replacing ' with '\''
  format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn nix_shell<'a, T>(
  path: &PathBuf,
  deps: Option<T>,
  cmds: &[String],
  inherit: bool,
  project_name: &str,
  project_dir: &Path,
) -> Command
where
  T: Iterator<Item = &'a String>,
{
  // Escape project_name and project_dir for bash
  let escaped_name = escape_bash_string(project_name);
  let escaped_dir = escape_bash_string(&project_dir.to_string_lossy());

  // Prepend environment variables to commands
  let env_prefix =
    format!("PROJECT_NAME={} PROJECT_DIR={} ", escaped_name, escaped_dir);
  let joined_cmds = cmds
    .iter()
    .map(|cmd| format!("{}{}", env_prefix, cmd))
    .collect::<Vec<_>>()
    .join("&&");

  if let Some(deps) = deps {
    let mut cmd = Command::new(NIX_SHELL_PATH.as_path());
    if inherit {
      cmd.stdout(Stdio::inherit());
      cmd.stderr(Stdio::inherit());
      cmd.stdin(Stdio::inherit());
    }

    cmd.current_dir(path);
    cmd.arg("-p").args(deps).arg("--run").arg(joined_cmds);
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
    cmd.arg("-c").arg(joined_cmds);
    cmd
  }
}
