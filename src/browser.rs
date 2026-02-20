use headless_chrome::{Browser, LaunchOptions};
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::process::Command;
use std::ffi::OsStr;
use std::env;
use crate::config::{USER_AGENT, CHROME_PATH};

fn find_chromium_path() -> Result<PathBuf> {
    let p1 = PathBuf::from(CHROME_PATH);
    if p1.exists() { return Ok(p1); }

    if let Ok(output) = Command::new("which").arg("chromium").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() { return Ok(PathBuf::from(s)); }
        }
    }
    Err(anyhow!("Chromium binary not found. Run: pkg install chromium"))
}

pub fn launch_browser() -> Result<Browser> {
    let termux_path = find_chromium_path()?;
    let ua_arg = format!("--user-agent={}", USER_AGENT);
    
    let random_id: u32 = rand::random();
    let temp_dir = std::env::temp_dir().join(format!("chrome_stov_{}", random_id));
    let user_data_arg = format!("--user-data-dir={}", temp_dir.to_string_lossy());

    let has_display = env::var("DISPLAY").is_ok();
    
    if has_display {
        println!(" [DISPLAY] Launching in X11 Desktop Visual Mode (1920x1080)...");
    } else {
        println!(" [HEADLESS] Launching in Invisible Desktop Mode.");
    }

    // DESKTOP ARGUMENTS
    let args_vec = vec![
        "--no-sandbox",
        "--disable-setuid-sandbox",
        "--disable-dev-shm-usage",
        "--disable-gpu",
        "--no-zygote",
        "--single-process",
        "--ignore-certificate-errors",
        // Force Desktop Resolution
        "--window-size=1920,1080", 
        "--start-maximized",
        "--disable-mobile-emulation",
        "--disable-software-rasterizer",
        "--disable-default-apps",
        "--disable-extensions",
        "--disable-sync",
        "--no-first-run",
        "--autoplay-policy=no-user-gesture-required",
        "--use-fake-ui-for-media-stream",
        &user_data_arg,
        &ua_arg
    ];

    let options = LaunchOptions {
        headless: !has_display, 
        sandbox: false,
        path: Some(termux_path),
        // Explicit Desktop Size in Options
        window_size: Some((1920, 1080)), 
        enable_gpu: false,
        args: args_vec.iter().map(|s| OsStr::new(s)).collect(),
        ..Default::default()
    };

    println!("Initializing Termux Chromium Engine (Desktop Edition)...");
    
    match Browser::new(options) {
        Ok(b) => Ok(b),
        Err(e) => Err(anyhow!("Browser Launch Failed: {}. \nTip: If using X11, ensure Termux-X11 app is open.", e))
    }
}
