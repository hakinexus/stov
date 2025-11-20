// Browser Identity
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36";

// Paths
pub const CHROME_PATH: &str = "/data/data/com.termux/files/usr/bin/chromium";
pub const DOWNLOAD_DIR: &str = "./downloads";
pub const IMAGES_DIR: &str = "./images";
pub const PROOF_DIR: &str = "./images/login_proofs";
pub const ERROR_DIR: &str = "./images/story_errors";

// --- Selectors ---
pub const USER_CSS: &str = "input[name='username']";
pub const USER_XPATH_1: &str = "//input[contains(@aria-label, 'username') or contains(@aria-label, 'Mobile')]";
pub const USER_XPATH_2: &str = "//input[@type='text']";

pub const PASS_CSS: &str = "input[name='password']";
pub const PASS_XPATH: &str = "//input[@type='password']";

pub const SEL_SUBMIT: &str = "button[type='submit']";
pub const SEL_HOME_ICON: &str = "svg[aria-label='Home']"; 
pub const SEL_STORY_RING: &str = "canvas"; 

// This was missing in your file:
pub const SEL_AVATAR: &str = "img[alt*='profile picture']";
