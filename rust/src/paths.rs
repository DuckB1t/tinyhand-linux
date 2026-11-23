use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nix::unistd::Uid;

fn expand_user(value: std::ffi::OsString) -> PathBuf {
    let path = PathBuf::from(value);
    if path == Path::new("~") {
        return dirs_home();
    }
    if let Ok(rest) = path.strip_prefix("~").and_then(|p| p.strip_prefix(std::path::MAIN_SEPARATOR.to_string())) {
        return dirs_home().join(rest);
    }
    path
}

fn xdg_dir(name: &str, default: &str) -> PathBuf {
    match env::var_os(name) {
        Some(value) if !value.is_empty() => expand_user(value),
        _ => dirs_home().join(default),
    }
}

fn dirs_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config").join("tinyhand-linux")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

pub fn data_dir() -> PathBuf {
    xdg_dir("XDG_DATA_HOME", ".local/share").join("tinyhand-linux")
}

pub fn runtime_dir() -> PathBuf {
    let path = match env::var_os("XDG_RUNTIME_DIR") {
        Some(value) if !value.is_empty() => expand_user(value),
        _ => PathBuf::from(format!("/tmp/tinyhand-{}", Uid::current().as_raw())),
    };
    let _ = fs::create_dir_all(&path);
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o700));
    path
}

pub fn control_socket() -> PathBuf {
    runtime_dir().join("tinyhand-linux.sock")
}

pub fn lock_file() -> PathBuf {
    runtime_dir().join("tinyhand-linux.lock")
}

pub fn state_file() -> PathBuf {
    runtime_dir().join("tinyhand-linux-state.json")
}

pub fn log_file() -> PathBuf {
    let path = xdg_dir("XDG_STATE_HOME", ".local/state")
        .join("tinyhand-linux")
        .join("tinyhand.log");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    path
}

pub fn asset_dir() -> PathBuf {
    if let Some(path) = env::var_os("TINYHAND_ASSETS") {
        return expand_user(path);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(bin) = exe.parent() {
            let installed = bin.join("../share/tinyhand-linux/assets");
            if installed.is_dir() {
                return installed;
            }
            let adjacent = bin.join("assets");
            if adjacent.is_dir() {
                return adjacent;
            }
        }
    }
    let local = data_dir().join("assets");
    if local.is_dir() {
        return local;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src/tinyhand_linux/assets")
}

pub fn autostart_file() -> PathBuf {
    dirs_home().join(".config/autostart/tinyhand-linux.desktop")
}

pub fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
}
