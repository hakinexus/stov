use headless_chrome::{Browser, Tab};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
use crate::config::*;
use crate::utils::{log_info, log_error, download_file};

pub struct InstagramBot<'a> {
    _browser: &'a Browser,
    tab: Arc<Tab>,
}

impl<'a> InstagramBot<'a> {
    pub fn new(browser: &'a Browser) -> Result<Self> {
        let tab = browser.new_tab()?;
        Ok(Self { _browser: browser, tab })
    }

    pub fn login(&self, user: &str, pass: &str) -> Result<()> {
        log_info("Navigating directly to Login Page...");
        self.tab.navigate_to("https://www.instagram.com/accounts/login/")?;
        
        thread::sleep(Duration::from_secs(10)); 

        // --- Cookie Popup Handler ---
        let cookie_xpaths = vec![
            "//button[contains(text(), 'Allow all cookies')]",
            "//button[contains(text(), 'Allow')]",
            "//button[contains(text(), 'Decline')]"
        ];

        for xpath in cookie_xpaths {
            if let Ok(el) = self.tab.find_element_by_xpath(xpath) {
                log_info("Cookie popup detected. Clicking...");
                let _ = el.click();
                thread::sleep(Duration::from_secs(2));
                break;
            }
        }
        
        log_info("Looking for Username field...");
        
        match self.tab.wait_for_element(SEL_USERNAME) {
            Ok(u_el) => {
                if let Err(e) = u_el.type_into(user) {
                     log_error(&format!("Failed to type username: {}", e));
                }
                thread::sleep(Duration::from_millis(1500));

                if let Ok(p_el) = self.tab.find_element(SEL_PASSWORD) {
                    let _ = p_el.type_into(pass);
                }
                thread::sleep(Duration::from_millis(1500));

                if let Ok(btn) = self.tab.find_element(SEL_SUBMIT) {
                    let _ = btn.click();
                }
            },
            Err(e) => {
                // Debug Screenshot
                if let Ok(png) = self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                    let _ = std::fs::write("debug_login_error.png", png);
                }
                return Err(anyhow!("Login Element missing. Error: {}", e));
            }
        }

        log_info("Credentials entered. Waiting for redirect...");
        thread::sleep(Duration::from_secs(15)); 

        // --- Verification & Proof ---
        match self.tab.wait_for_element(SEL_HOME_ICON) {
            Ok(_) => {
                log_info("Login Verified: Home Icon detected.");
                // TAKE A SCREENSHOT SO THE USER CAN SEE THEY ARE LOGGED IN
                log_info("Saving login proof to 'login_proof.png'...");
                if let Ok(png) = self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                    let _ = std::fs::write("login_proof.png", png);
                }
            },
            Err(_) => {
                let url = self.tab.get_url();
                if url.contains("accounts/login") {
                    if let Ok(png) = self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                        let _ = std::fs::write("login_failed.png", png);
                    }
                    return Err(anyhow!("Login Failed. See login_failed.png"));
                } else {
                    log_info("Login Assumed (URL changed). Saving proof...");
                    if let Ok(png) = self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                        let _ = std::fs::write("login_proof.png", png);
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn process_targets(&self, targets: Vec<String>) -> Result<()> {
        for target in targets {
            log_info(&format!("Checking target: {}", target));
            
            let url = format!("https://www.instagram.com/{}/", target);
            if let Err(e) = self.tab.navigate_to(&url) {
                log_error(&format!("Failed to navigate: {}", e));
                continue;
            }
            
            thread::sleep(Duration::from_secs(8)); 

            if self.tab.find_element(SEL_STORY_RING).is_ok() {
                log_info("Story found! Opening...");
                if let Err(e) = self.capture_story(&target).await {
                    log_error(&format!("Error capturing story: {}", e));
                }
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
            el.click()?;
        }
        
        // Wait for story player to load fully
        thread::sleep(Duration::from_secs(5));

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        
        // --- AGGRESSIVE EXTRACTION STRATEGY ---
        // We check multiple potential locations for the media URL.
        
        let mut media_found = false;

        // 1. Try finding a VIDEO first (highest priority)
        if let Ok(video) = self.tab.find_element("video") {
             // Try 'src' attribute
             if let Ok(Some(src)) = video.get_attribute_value("src") {
                 if src.starts_with("http") {
                     log_info("Video detected!");
                     let fname = format!("{}_{}.mp4", username, timestamp);
                     download_file(&src, &fname).await?;
                     media_found = true;
                 }
             } 
             // Try 'source' child tag if src wasn't on video tag
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
        
        // 2. If no video, try finding an IMAGE
        if !media_found {
            // Try generic img inside the story viewer section
            // We look for an image that is NOT the profile picture (usually small)
            if let Ok(imgs) = self.tab.find_elements("img") {
                for img in imgs {
                    // We prefer srcset, but accept src if it looks like a CDN link
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
                    // Fallback to simple src if it looks large/valid
                    if !media_found {
                        if let Ok(Some(src)) = img.get_attribute_value("src") {
                            if src.contains("1080x") || src.contains("story") || src.len() > 100 {
                                log_info("Standard Image detected!");
                                let fname = format!("{}_{}.jpg", username, timestamp);
                                download_file(&src, &fname).await?;
                                media_found = true;
                                break;
                            }
                        }
                    }
                }
            }
        }

        if !media_found {
            log_error("Extraction Failed: Media loaded, but URL format was unrecognized.");
            // Snapshot the story so we can see what went wrong
            if let Ok(png) = self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
                let _ = std::fs::write("story_error.png", png);
            }
        }

        // Close story view
        let _ = self.tab.press_key("Escape");
        thread::sleep(Duration::from_secs(2));

        Ok(())
    }
}
