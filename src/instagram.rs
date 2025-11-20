use headless_chrome::{Browser, Tab, Element};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
use crate::config::*;
use crate::utils::{log_info, log_error, save_base64_file, save_screenshot, save_html};

pub struct InstagramBot<'a> {
    _browser: &'a Browser,
    tab: Arc<Tab>,
}

impl<'a> InstagramBot<'a> {
    pub fn new(browser: &'a Browser) -> Result<Self> {
        let tab = browser.new_tab()?;
        Ok(Self { _browser: browser, tab })
    }

    fn smart_find(&self, css: &str, xpath1: &str, xpath2: Option<&str>) -> Result<Element<'_>> {
        if let Ok(el) = self.tab.find_element(css) { return Ok(el); }
        if let Ok(el) = self.tab.find_element_by_xpath(xpath1) { return Ok(el); }
        if let Some(x2) = xpath2 {
            if let Ok(el) = self.tab.find_element_by_xpath(x2) { return Ok(el); }
        }
        Err(anyhow!("Element not found"))
    }

    fn snapshot(&self, folder: &str, name: &str) {
        match self.tab.capture_screenshot(headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png, None, None, true) {
            Ok(png) => { let _ = save_screenshot(png, folder, name); },
            Err(_) => { if let Ok(c) = self.tab.get_content() { save_html(c, folder, name); } }
        }
    }

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
        thread::sleep(Duration::from_secs(6)); 

        let cookie_xpaths = vec!["//button[contains(text(), 'Allow all cookies')]", "//button[contains(text(), 'Allow')]", "//button[contains(text(), 'Decline')]"];
        for xpath in cookie_xpaths {
            if let Ok(el) = self.tab.find_element_by_xpath(xpath) { let _ = el.click(); thread::sleep(Duration::from_secs(1)); break; }
        }
        
        log_info("Inputting Credentials...");
        match self.smart_find(USER_CSS, USER_XPATH_1, Some(USER_XPATH_2)) {
            Ok(u_el) => { if let Err(e) = self.react_type(&u_el, user) { log_error(&format!("User Type Error: {}", e)); } },
            Err(e) => { self.snapshot(ERROR_DIR, "missing_username"); return Err(e); }
        }
        thread::sleep(Duration::from_millis(500));
        match self.smart_find(PASS_CSS, PASS_XPATH, None) {
            Ok(p_el) => { if let Err(e) = self.react_type(&p_el, pass) { log_error(&format!("Pass Type Error: {}", e)); } },
            Err(e) => { self.snapshot(ERROR_DIR, "missing_password"); return Err(e); }
        }
        thread::sleep(Duration::from_millis(500));
        if let Ok(btn) = self.tab.find_element(SEL_SUBMIT) { let _ = btn.click(); } else { let _ = self.tab.press_key("Enter"); }

        log_info("Verifying authentication...");
        let start_time = Instant::now();
        loop {
            if self.tab.find_element(SEL_HOME_ICON).is_ok() || self.tab.find_element(SEL_AVATAR).is_ok() {
                log_info("Login Verified.");
                self.snapshot(PROOF_DIR, "login_success");
                return Ok(());
            }
            if let Ok(el) = self.tab.find_element_by_xpath("//button[contains(text(), 'Not Now')]") {
                 let _ = el.click();
                 return Ok(());
            }
            let url = self.tab.get_url();
            if !url.contains("accounts/login") && !url.contains("challenge") && url.len() > 20 {
                log_info("Login Assumed (URL changed).");
                thread::sleep(Duration::from_secs(5));
                return Ok(());
            }
            if start_time.elapsed() > Duration::from_secs(60) {
                 self.snapshot(PROOF_DIR, "login_timeout");
                 return Err(anyhow!("Login Timed Out"));
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    pub async fn process_targets(&self, targets: Vec<String>) -> Result<()> {
        for target in targets {
            log_info(&format!("Checking target: {}", target));
            let url = format!("https://www.instagram.com/{}/", target);
            if let Err(_) = self.tab.navigate_to(&url) { continue; }
            thread::sleep(Duration::from_secs(6)); 

            if self.tab.find_element(SEL_STORY_RING).is_ok() {
                log_info("Story found! Opening...");
                let _ = self.capture_story(&target).await;
            } else {
                log_info("No story found.");
            }
            thread::sleep(Duration::from_secs(rand::thread_rng().gen_range(3..7)));
        }
        Ok(())
    }

    async fn capture_story(&self, username: &str) -> Result<()> {
        if let Ok(el) = self.tab.find_element(SEL_STORY_RING) { let _ = el.click(); }
        
        thread::sleep(Duration::from_secs(4)); 
        
        log_info("Monitoring Network for Large Media...");
        let start_time = Instant::now();
        let mut tried_urls: Vec<String> = Vec::new();

        loop {
            if start_time.elapsed() > Duration::from_secs(45) {
                log_error("Timeout: No valid large media files found.");
                self.snapshot(ERROR_DIR, "story_timeout");
                let _ = self.tab.press_key("Escape");
                return Ok(());
            }

            // --- EXPERT JS: SIZE-BASED FILTERING ---
            // We check the browser's internal Performance Logs for transferSize.
            // We only accept MP4s > 300KB or JPGs > 50KB.
            let js_identify = r#"
                (function() {
                    let valid_urls = [];
                    
                    // 1. Check Network Logs (The most accurate)
                    let entries = performance.getEntriesByType('resource');
                    for (let i = entries.length - 1; i >= 0; i--) {
                        let e = entries[i];
                        // MP4 Check: Must be > 300KB (300000 bytes)
                        if (e.name.includes('.mp4') && e.transferSize > 300000) {
                            valid_urls.push(e.name);
                        }
                        // JPG Check: Must be > 50KB (50000 bytes)
                        if (e.name.includes('.jpg') && e.name.includes('instagram') && e.transferSize > 50000) {
                             valid_urls.push(e.name);
                        }
                    }

                    // 2. DOM Check (Fallback)
                    let v = document.querySelector('video');
                    if (v && v.src && v.src.startsWith('http')) valid_urls.push(v.src);

                    let imgs = Array.from(document.querySelectorAll('img'));
                    let target = imgs.find(i => i.srcset && i.src.includes('instagram'));
                    if (target) {
                         let parts = target.srcset.split(',');
                         valid_urls.push(parts[parts.length - 1].trim().split(' ')[0]);
                    }
                    
                    return [...new Set(valid_urls)].join(';');
                })()
            "#;

            if let Ok(res) = self.tab.evaluate(js_identify, false) {
                 if let Some(val) = res.value {
                     let s = val.as_str().unwrap_or("");
                     let candidates: Vec<&str> = s.split(';').collect();
                     
                     for c in candidates {
                         if c.len() < 10 || tried_urls.contains(&c.to_string()) { continue; }
                         if c.contains("blob:") { continue; } // Skip blobs for now, focus on clean URLs

                         // Strip range params (attempt to un-segment)
                         let mut clean_url = c.to_string();
                         if let Some(idx) = clean_url.find("&bytestart") { clean_url = clean_url[..idx].to_string(); }

                         let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                         let ext = if clean_url.contains(".mp4") { "mp4" } else { "jpg" };
                         let fname = format!("{}_{}.{}", username, timestamp, ext);

                         log_info(&format!("Candidate Found ({}) - Attempting Fetch...", ext));
                         tried_urls.push(c.to_string());

                         // BROWSER FETCH
                         let js_fetch = format!(r#"
                            (async function() {{
                                try {{
                                    const response = await fetch("{}", {{ cache: 'reload' }}); // Force new request
                                    const blob = await response.blob();
                                    return await new Promise((resolve) => {{
                                        const reader = new FileReader();
                                        reader.onloadend = () => resolve(reader.result);
                                        reader.readAsDataURL(blob);
                                    }});
                                }} catch (err) {{ return "ERROR"; }}
                            }})()
                         "#, clean_url);

                         match self.tab.evaluate(&js_fetch, true) {
                            Ok(res_fetch) => {
                                if let Some(data_val) = res_fetch.value {
                                    let data_uri = data_val.as_str().unwrap_or("");
                                    if data_uri.starts_with("data:") {
                                        // Send to Utils.rs (Checks for >500KB)
                                        if let Ok(_) = save_base64_file(data_uri, &fname) {
                                            let _ = self.tab.press_key("Escape");
                                            return Ok(());
                                        } else {
                                            log_error("File validation failed (Too small). Continuing scan...");
                                        }
                                    }
                                }
                            },
                            Err(_) => {}
                         }
                     }
                 }
            }
            thread::sleep(Duration::from_millis(1000));
        }
    }
}
