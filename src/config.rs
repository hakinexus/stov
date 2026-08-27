// Browser and runtime defaults
pub const CHROME_PATH: &str = "/data/data/com.termux/files/usr/bin/chromium";

// Filesystem layout
pub const DOWNLOAD_DIR: &str = "./downloads";
pub const IMAGES_DIR: &str = "./images";
pub const PROOF_DIR: &str = "./images/login_proofs";
pub const ERROR_DIR: &str = "./images/story_errors";
pub const PROFILES_DIR: &str = "./profiles";

// Login selectors. Instagram has used several equivalent attributes over time.
pub const USER_CSS_SELECTORS: &[&str] = &[
    "input[name='username']",
    "input[autocomplete='username']",
    "input[aria-label='Phone number, username, or email']",
    "input[aria-label*='username' i]",
    "input[placeholder*='username' i]",
    "input[type='text']",
    "input:not([type])",
];
pub const USER_XPATH_SELECTORS: &[&str] = &[
    "//input[@name='username']",
    "//input[@autocomplete='username']",
    "//input[contains(translate(@aria-label, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), 'username')]",
    "//input[contains(translate(@placeholder, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), 'username')]",
];
pub const PASS_CSS_SELECTORS: &[&str] = &[
    "input[name='password']",
    "input[name='pass']",
    "input[type='password']",
    "input[autocomplete='current-password']",
    "input[aria-label='Password']",
    "input[aria-label*='password' i]",
    "input[placeholder*='password' i]",
    "input[id*='password' i]",
];
pub const PASS_XPATH_SELECTORS: &[&str] = &[
    "//input[@name='password']",
    "//input[@name='pass']",
    "//input[@type='password']",
    "//input[@autocomplete='current-password']",
    "//input[contains(translate(@aria-label, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), 'password')]",
    "//input[contains(translate(@placeholder, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), 'password')]",
    "//input[contains(translate(@id, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), 'password')]",
];
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
