use headless_chrome::{Browser, Tab, Element};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant}; // Added Instant
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
use crate::config::*;
use crate::utils::{log_info, log_error, download_file, save_screenshot, save_html};

pub struct InstagramBot<'a> {
    _browser: &'a Browser,
    tab: Arc<Tab>,
}

impl<'a> InstagramBot<'a> {
    pub fn new(browser: &'a Browser) -> Result<Self> {
        let tab = browser.new_tab()?;
        Ok(Self { _browser: browser, tab })
    }

    // --- HELPER: SMART FINDER ---
    fn smart_find(&self, css: &str, xpath1: &str, xpath2: Option<&str>) -> Result<Element<'_>> {
        if let Ok(el) = self.tab.find_element(css) { return Ok(el); }
        if let Ok(el) = self.tab.find_element_by_xpath(xpath1) { return Ok(el); }
        if let Some(x2) = xpath2 {
            if let Ok(el) = self.tab.find_element_by_xpath(x2) { return Ok(el); }
        }
        Err(anyhow!("Element not found"))
    }

    // --- HELPER: FAIL-SAFE SNAPSHOT ---
    fn snapshot(&self, folder: &str, name: &str) {
        match self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
            Ok(png) => {
                let _ = save_screenshot(png, folder, name);
            },
            Err(e) => {
                log_error(&format!("Screenshot failed: {}", e));
                if let Ok(content) = self.tab.get_content() {
                    save_html(content, folder, name);
                }
            }
        }
    }

    // --- HELPER: REACT TYPING ---
    fn react_type(&self, el: &Element, text: &str) -> Result<()> {
        el.click()?; 
        el.type_into(text)?;
        thread::sleep(Duration::from_millis(500));
        let _ = self.tab.press_key("Tab"); 
        Ok(())
    }

    pub fn login(&self, user: &str, pass: &str) -> Result<()> {
        log_info("Navigating directly to Login Page...");
        self.tab.navigate_to("https://www.instagram.com/accounts/login/")?;
        
        // Initial load wait
        thread::sleep(Duration::from_secs(8)); 

        // Cookie Handler
        let cookie_xpaths = vec![
            "//button[contains(text(), 'Allow all cookies')]",
            "//button[contains(text(), 'Allow')]",
            "//button[contains(text(), 'Decline')]"
        ];
        for xpath in cookie_xpaths {
            if let Ok(el) = self.tab.find_element_by_xpath(xpath) {
                let _ = el.click();
                thread::sleep(Duration::from_secs(1));
                break;
            }
        }
        
        log_info("Inputting Credentials...");

        // 1. Username
        match self.smart_find(USER_CSS, USER_XPATH_1, Some(USER_XPATH_2)) {
            Ok(u_el) => {
                if let Err(e) = self.react_type(&u_el, user) {
                    log_error(&format!("Error typing username: {}", e));
                }
            },
            Err(e) => {
                self.snapshot(ERROR_DIR, "missing_username");
                return Err(e);
            }
        }
        
        thread::sleep(Duration::from_millis(1000));

        // 2. Password
        match self.smart_find(PASS_CSS, PASS_XPATH, None) {
            Ok(p_el) => {
                 if let Err(e) = self.react_type(&p_el, pass) {
                     log_error(&format!("Error typing password: {}", e));
                 }
            },
            Err(e) => {
                self.snapshot(ERROR_DIR, "missing_password");
                return Err(e);
            }
        }

        thread::sleep(Duration::from_millis(1000));

        // 3. Click Login
        log_info("Clicking Login...");
        if let Ok(btn) = self.tab.find_element(SEL_SUBMIT) {
            let _ = btn.click();
        } else {
            let _ = self.tab.press_key("Enter");
        }

        // --- SMART POLLING LOOP (The Fix) ---
        log_info("Waiting for authentication (Max 60s)...");
        
        let start_time = Instant::now();
        let max_wait = Duration::from_secs(60); // Give it 1 minute to load

        loop {
            // Check 1: Success (Home Icon)
            if self.tab.find_element(SEL_HOME_ICON).is_ok() {
                log_info("Login Verified: Home Icon detected.");
                self.snapshot(PROOF_DIR, "login_success");
                return Ok(());
            }

            // Check 2: Success (URL Change)
            let url = self.tab.get_url();
            if !url.contains("accounts/login") {
                log_info("Login Assumed (URL changed).");
                // Wait a few seconds for the new page to render partially
                thread::sleep(Duration::from_secs(5));
                self.snapshot(PROOF_DIR, "login_redirect");
                return Ok(());
            }

            // Check 3: Explicit Errors (Incorrect Password)
            // We check this in the loop so we can fail fast if we see it
            let error_selectors = vec!["#slerror", "p[data-testid='login-error-message']", "p[role='alert']"];
            for sel in error_selectors {
                if let Ok(el) = self.tab.find_element(sel) {
                    if let Ok(text) = el.get_inner_text() {
                        log_error(&format!("LOGIN ERROR DETECTED: '{}'", text));
                        self.snapshot(PROOF_DIR, "login_rejected");
                        return Err(anyhow!("Instagram rejected credentials: {}", text));
                    }
                }
            }

            // Timeout Check
            if start_time.elapsed() > max_wait {
                log_error("Login Timed Out (Spinner stuck).");
                self.snapshot(PROOF_DIR, "login_timeout");
                return Err(anyhow!("Login process took too long (>60s)."));
            }

            // Wait 2 seconds before checking again
            thread::sleep(Duration::from_secs(2));
        }
    }

    pub async fn process_targets(&self, targets: Vec<String>) -> Result<()> {
        for target in targets {
            log_info(&format!("Checking target: {}", target));
            
            let url = format!("https://www.instagram.com/{}/", target);
            if let Err(_) = self.tab.navigate_to(&url) { continue; }
            
            thread::sleep(Duration::from_secs(8)); 

            if self.tab.find_element(SEL_STORY_RING).is_ok() {
                log_info("Story found! Opening...");
                let _ = self.capture_story(&target).await;
            } else {
                log_info("No story found.");
            }
            
            let delay = rand::thread_rng().gen_range(5..10);
            thread::sleep(Duration::from_secs(delay));
        }
        Ok(())
    }

    async fn capture_story(&self, username: &str) -> Result<()> {
        if let Ok(el) = self.tab.find_element(SEL_STORY_RING) {
            let _ = el.click();
        }
        
        thread::sleep(Duration::from_secs(5));

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let mut media_found = false;

        // 1. Try VIDEO
        if let Ok(video) = self.tab.find_element("video") {
             if let Ok(Some(src)) = video.get_attribute_value("src") {
                 if src.starts_with("http") {
                     log_info("Video detected!");
                     let fname = format!("{}_{}.mp4", username, timestamp);
                     download_file(&src, &fname).await?;
                     media_found = true;
                 }
             } 
             if !media_found {
                 if let Ok(source) = self.tab.find_element("video source") {
                    if let Ok(Some(src)) = source.get_attribute_value("src") {
                        log_info("Video source detected!");
                        let fname = format!("{}_{}.mp4", username, timestamp);
                        download_file(&src, &fname).await?;
                        media_found = true;
                    }
                 }
             }
        } 
        
        // 2. Try IMAGE
        if !media_found {
            if let Ok(imgs) = self.tab.find_elements("img") {
                for img in imgs {
                    if let Ok(Some(srcset)) = img.get_attribute_value("srcset") {
                        let parts: Vec<&str> = srcset.split(',').collect();
                        if let Some(last_part) = parts.last() {
                             let url = last_part.trim().split(' ').next().unwrap_or("");
                             if url.contains("instagram") || url.contains("cdn") {
                                 log_info("High-Res Image detected!");
                                 let fname = format!("{}_{}.jpg", username, timestamp);
                                 download_file(url, &fname).await?;
                                 media_found = true;
                                 break;
                             }
                        }
                    }
                }
            }
        }

        if !media_found {
            log_error("Media extraction failed.");
            self.snapshot(ERROR_DIR, "story_error");
        }

        let _ = self.tab.press_key("Escape");
        thread::sleep(Duration::from_secs(2));

        Ok(())
    }
}
