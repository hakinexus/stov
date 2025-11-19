// We fake a Windows 10 PC to ensure we get the standard login page, not the mobile app landing page
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";
pub const DOWNLOAD_DIR: &str = "./downloads";

// Chromium Path on Termux (Standard + Fallbacks are handled in browser.rs now)
pub const CHROME_PATH: &str = "/data/data/com.termux/files/usr/bin/chromium";

// Selectors (CSS)
pub const SEL_USERNAME: &str = "input[name='username']";
pub const SEL_PASSWORD: &str = "input[name='password']";
pub const SEL_SUBMIT: &str = "button[type='submit']";
pub const SEL_HOME_ICON: &str = "svg[aria-label='Home']"; 
pub const SEL_STORY_RING: &str = "canvas";
