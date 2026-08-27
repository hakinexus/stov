use anyhow::{anyhow, Result};
use headless_chrome::{Browser, LaunchOptions};
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::CHROME_PATH;

const CHROMIUM_NAMES: &[&str] = &["chromium", "chromium-browser", "google-chrome", "chrome"];

fn is_candidate_file(path: &Path) -> bool {
    path.is_file()
}

fn path_entries(path_value: Option<OsString>) -> Vec<PathBuf> {
    path_value
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn candidate_paths(prefix: Option<&Path>, path_value: Option<OsString>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(prefix) = prefix {
        candidates.extend(
            CHROMIUM_NAMES
                .iter()
                .map(|name| prefix.join("bin").join(name)),
        );
    }

    // Termux’s normal default prefix plus common Linux locations make the binary
    // discoverable even when `which` is unavailable or behaves differently.
    for path in [
        PathBuf::from(CHROME_PATH),
        PathBuf::from("/data/data/com.termux/files/usr/bin/chromium"),
        PathBuf::from("/data/data/com.termux/files/usr/bin/chromium-browser"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/local/bin/chromium"),
    ] {
        candidates.push(path);
    }

    for directory in path_entries(path_value) {
        candidates.extend(CHROMIUM_NAMES.iter().map(|name| directory.join(name)));
    }

    candidates
}

fn is_termux(prefix: Option<&Path>) -> bool {
    env::var_os("TERMUX_VERSION").is_some()
        || env::var_os("TERMUX_PREFIX").is_some()
        || prefix
            .map(|value| value.to_string_lossy().contains("/com.termux/"))
            .unwrap_or(false)
}

fn find_chromium_path() -> Result<PathBuf> {
    if let Ok(configured) = env::var("STOV_CHROMIUM_PATH") {
        let path = PathBuf::from(configured.trim());
        if is_candidate_file(&path) {
            return Ok(path);
        }
        return Err(anyhow!(
            "STOV_CHROMIUM_PATH is set but is not a file: {}. Run `command -v chromium-browser` and set STOV_CHROMIUM_PATH to that result.",
            path.display()
        ));
    }

    let prefix = env::var_os("PREFIX")
        .or_else(|| env::var_os("TERMUX_PREFIX"))
        .map(PathBuf::from);
    let candidates = candidate_paths(prefix.as_deref(), env::var_os("PATH"));
    if let Some(path) = candidates.iter().find(|path| is_candidate_file(path)) {
        return Ok(path.clone());
    }

    let searched = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    Err(anyhow!(
        "Chromium binary not found. STOV searched:\n  {}\n\nTermux setup:\n  pkg update\n  pkg install x11-repo tur-repo\n  pkg install chromium\n  command -v chromium-browser\n  chromium-browser --version\n\nIf Chromium is installed elsewhere, run:\n  export STOV_CHROMIUM_PATH=$(command -v chromium-browser)",
        searched
    ))
}

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value >= 320)
        .unwrap_or(default)
}

fn env_f32(name: &str, default: f32) -> f32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| (0.5..=4.0).contains(value))
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name).as_deref() {
        Ok("1") | Ok("true") | Ok("yes") => true,
        Ok("0") | Ok("false") | Ok("no") => false,
        _ => default,
    }
}

pub fn launch_browser() -> Result<Browser> {
    let chromium_path = find_chromium_path()?;
    let width = env_u32("STOV_WINDOW_WIDTH", 1920);
    let height = env_u32("STOV_WINDOW_HEIGHT", 1080);
    let scale_factor = env_f32("STOV_DEVICE_SCALE_FACTOR", 1.0);
    let display_available = env::var("DISPLAY").is_ok();
    let headless = env_bool("STOV_HEADLESS", !display_available);
    let user_data_dir = env::temp_dir().join(format!("stov-chrome-{}", rand::random::<u64>()));

    let mut args = vec![
        "--disable-dev-shm-usage".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--no-first-run".to_string(),
        "--autoplay-policy=no-user-gesture-required".to_string(),
        format!("--window-size={},{}", width, height),
        format!("--force-device-scale-factor={}", scale_factor),
        "--high-dpi-support=1".to_string(),
        "--force-color-profile=srgb".to_string(),
        "--window-position=0,0".to_string(),
        format!("--user-data-dir={}", user_data_dir.display()),
    ];

    // Termux normally requires these two flags; keep them opt-in elsewhere.
    let prefix = env::var_os("PREFIX")
        .or_else(|| env::var_os("TERMUX_PREFIX"))
        .map(PathBuf::from);
    let termux = is_termux(prefix.as_deref());

    if env::var("STOV_ALLOW_NO_SANDBOX").as_deref() == Ok("1") || termux {
        args.push("--no-sandbox".to_string());
        args.push("--disable-setuid-sandbox".to_string());
    }

    if let Ok(user_agent) = env::var("STOV_USER_AGENT") {
        if !user_agent.trim().is_empty() {
            args.push(format!("--user-agent={}", user_agent));
        }
    }

    let arg_refs: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let options = LaunchOptions {
        headless,
        sandbox: env::var("STOV_ALLOW_NO_SANDBOX").as_deref() != Ok("1") && !termux,
        path: Some(chromium_path.clone()),
        window_size: Some((width, height)),
        enable_gpu: env::var("STOV_ENABLE_GPU").as_deref() == Ok("1"),
        args: arg_refs,
        ..Default::default()
    };

    println!(
        "Launching Chromium at {} ({}x{}, {})...",
        chromium_path.display(),
        width,
        height,
        if options.headless {
            "headless"
        } else {
            "display"
        }
    );

    Command::new(&chromium_path)
        .arg("--version")
        .output()
        .map_err(|error| {
            anyhow!(
                "Chromium was found at {} but could not execute it: {}",
                chromium_path.display(),
                error
            )
        })?;

    Browser::new(options).map_err(|error| {
        anyhow!(
            "Browser launch failed using {}: {}. If this is Termux, confirm `chromium-browser --version` works and set STOV_ALLOW_NO_SANDBOX=1 only when required.",
            chromium_path.display(),
            error
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_include_termux_prefix_and_path_entries() {
        let paths = candidate_paths(
            Some(Path::new("/data/data/com.termux/files/usr")),
            Some(OsString::from("/custom/bin:/another/bin")),
        );
        assert!(paths
            .iter()
            .any(|path| path == Path::new("/data/data/com.termux/files/usr/bin/chromium")));
        assert!(paths
            .iter()
            .any(|path| path == Path::new("/custom/bin/chromium")));
        assert!(paths
            .iter()
            .any(|path| path == Path::new("/another/bin/google-chrome")));
    }
}
