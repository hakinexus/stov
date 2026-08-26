// Browser and runtime defaults
pub const CHROME_PATH: &str = "/data/data/com.termux/files/usr/bin/chromium";

// Filesystem layout
pub const DOWNLOAD_DIR: &str = "./downloads";
pub const IMAGES_DIR: &str = "./images";
pub const PROOF_DIR: &str = "./images/login_proofs";
pub const ERROR_DIR: &str = "./images/story_errors";
pub const PROFILES_DIR: &str = "./profiles";

// Login selectors
pub const USER_CSS: &str = "input[name='username']";
pub const USER_XPATH_1: &str = "//input[@name='username']";
pub const USER_XPATH_2: &str = "//input[@type='text']";
pub const PASS_CSS: &str = "input[name='password']";
pub const PASS_XPATH: &str = "//input[@name='password']";
pub const SEL_SUBMIT: &str = "button[type='submit']";
pub const SEL_HOME_ICON: &str = "svg[aria-label='Home']";
pub const SEL_AVATAR: &str = "img[alt*='profile picture']";

// Instagram changes its DOM frequently. Keep semantic fallbacks and log the selector that matched.
pub const STORY_ENTRY_SELECTORS: &[&str] = &[
    "a[href*='/stories/']",
    "main header canvas",
    "header canvas",
    "main header img",
    "img[alt*='profile picture']",
];

pub const STORY_NEXT_SELECTORS: &[&str] = &[
    "button[aria-label='Next']",
    "button[aria-label='Next story']",
    "div[role='button'][aria-label='Next']",
    "div[role='button'][aria-label='Next story']",
];
