use anyhow::{anyhow, Result};
use headless_chrome::{Browser, LaunchOptions};
use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use crate::config::CHROME_PATH;

fn find_chromium_path() -> Result<PathBuf> {
    if let Ok(configured) = env::var("STOV_CHROMIUM_PATH") {
        let path = PathBuf::from(configured);
        if path.exists() {
            return Ok(path);
        }
        return Err(anyhow!(
            "STOV_CHROMIUM_PATH does not exist: {}",
            path.display()
        ));
    }

    let configured_path = PathBuf::from(CHROME_PATH);
    if configured_path.exists() {
        return Ok(configured_path);
    }

    if let Ok(output) = Command::new("which").arg("chromium").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    Err(anyhow!(
        "Chromium binary not found. Set STOV_CHROMIUM_PATH or install Chromium."
    ))
}

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value >= 320)
        .unwrap_or(default)
}

pub fn launch_browser() -> Result<Browser> {
    let chromium_path = find_chromium_path()?;
    let width = env_u32("STOV_WINDOW_WIDTH", 1280);
    let height = env_u32("STOV_WINDOW_HEIGHT", 720);
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
        format!("--user-data-dir={}", user_data_dir.display()),
    ];

    // Termux normally requires these two flags; keep them opt-in elsewhere.
    if env::var("STOV_ALLOW_NO_SANDBOX").as_deref() == Ok("1") || env::var("TERMUX_VERSION").is_ok()
    {
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
        headless: env::var("DISPLAY").is_err(),
        sandbox: env::var("STOV_ALLOW_NO_SANDBOX").as_deref() != Ok("1")
            && env::var("TERMUX_VERSION").is_err(),
        path: Some(chromium_path),
        window_size: Some((width, height)),
        enable_gpu: env::var("STOV_ENABLE_GPU").as_deref() == Ok("1"),
        args: arg_refs,
        ..Default::default()
    };

    println!(
        "Launching Chromium ({}x{}, {})...",
        width,
        height,
        if options.headless {
            "headless"
        } else {
            "display"
        }
    );

    Browser::new(options).map_err(|error| {
        anyhow!(
            "Browser launch failed: {}. Set STOV_CHROMIUM_PATH if Chromium is not on PATH.",
            error
        )
    })
}
