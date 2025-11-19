use headless_chrome::{Browser, LaunchOptions};
use anyhow::Result;
use std::path::PathBuf;
use std::ffi::OsStr;
use crate::config::{CHROME_PATH, USER_AGENT};

pub fn launch_browser() -> Result<Browser> {
    let termux_path = PathBuf::from(CHROME_PATH);

    // We format the user agent as a command line flag
    let ua_arg = format!("--user-agent={}", USER_AGENT);

    // These arguments are MANDATORY for Termux stability
    let args_vec = vec![
        "--no-sandbox",               // Android kernel doesn't support sandbox
        "--disable-setuid-sandbox",   
        "--disable-dev-shm-usage",    // Prevents memory crashes
        "--disable-gpu",              // Saves resources
        "--no-zygote",                // Simplifies process model
        "--single-process",           // Reduces RAM usage
        "--ignore-certificate-errors",
        "--window-size=1280,720",
        &ua_arg // Add the User Agent here
    ];

    let options = LaunchOptions {
        headless: true, 
        sandbox: false,
        path: Some(termux_path),
        window_size: Some((1280, 720)),
        // user_agent field removed (moved to args above)
        enable_gpu: false,
        args: args_vec.iter().map(|s| OsStr::new(s)).collect(),
        ..Default::default()
    };

    println!("Initializing Termux Chromium Engine...");
    let browser = Browser::new(options)?;
    Ok(browser)
}
