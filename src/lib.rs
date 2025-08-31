use std::{path::PathBuf, str::FromStr, sync::LazyLock};

pub static SELF_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
  // Absolute path to the currently running executable.
  // On Linux this resolves /proc/self/exe to a real path.
  let exe = std::env::current_exe().expect("cannot get current exe");
  // If it's a symlink, canonicalize to the real file (best-effort).
  exe.canonicalize().unwrap_or(exe)
});

// pub static NIX_SHELL_PATH: LazyLock<PathBuf> =
//   LazyLock::new(|| which::which("nix-shell").unwrap());
pub static NIX_SHELL_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
  PathBuf::from_str("/nix/var/nix/profiles/default/bin/nix-shell").unwrap()
});

pub static IS_SAFE_MODE: LazyLock<bool> =
  LazyLock::new(|| std::env::var_os("MODE").is_some_and(|v| v == "safe"));

pub mod action;
pub mod instance;
pub mod project;
