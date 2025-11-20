use headless_chrome::{Browser, Tab, Element};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
use std::collections::HashSet; 
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

    fn inject_sniffer(&self) {
        let script = r#"
            if (!window.__sniffer_active) {
                window.__intercepted_urls = [];
                const observer = new PerformanceObserver((list) => {
                    list.getEntries().forEach((entry) => {
                        if (entry.name.includes('.mp4') || (entry.name.includes('.jpg') && entry.name.includes('instagram'))) {
                            window.__intercepted_urls.push(entry.name);
                        }
                    });
                });
                observer.observe({ entryTypes: ['resource'] });
                window.__sniffer_active = true;
            }
        "#;
        let _ = self.tab.evaluate(script, false);
    }

    fn clear_network_logs(&self) {
        let script = "window.__intercepted_urls = []; performance.clearResourceTimings();";
        let _ = self.tab.evaluate(script, false);
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
            thread::sleep(Duration::from_secs(5)); 

            if self.tab.find_element(SEL_STORY_RING).is_ok() {
                log_info("Story found! Starting batch download...");
                let _ = self.process_story_batch(&target).await;
            } else {
                log_info("No stories found for this user.");
            }
            thread::sleep(Duration::from_secs(rand::thread_rng().gen_range(3..6)));
        }
        Ok(())
    }

    async fn process_story_batch(&self, username: &str) -> Result<()> {
        self.inject_sniffer();
        
        if let Ok(el) = self.tab.find_element(SEL_STORY_RING) { let _ = el.click(); }
        
        thread::sleep(Duration::from_secs(3));

        let mut downloaded_history: HashSet<String> = HashSet::new();
        let mut story_count = 0;
        let mut consecutive_errors = 0;

        log_info(&format!("Starting batch extraction for: {}", username));
        self.clear_network_logs();

        loop {
            let current_url = self.tab.get_url();
            if !current_url.contains("stories") {
                log_info("Batch ended (Returned to feed).");
                break;
            }
            if !current_url.contains(username) {
                log_info("Batch ended (Moved to different user).");
                let _ = self.tab.press_key("Escape");
                break;
            }

            match self.download_active_story(username, &mut downloaded_history).await {
                Ok(true) => {
                    story_count += 1;
                    consecutive_errors = 0;
                    log_info(&format!("Story #{} Saved.", story_count));
                    self.clear_network_logs(); 
                    log_info("Moving to next...");
                    let _ = self.tab.press_key("ArrowRight");
                    thread::sleep(Duration::from_millis(1500));
                },
                Ok(false) => {
                    consecutive_errors += 1;
                    log_info("Skipping (No new media found)...");
                    let _ = self.tab.press_key("ArrowRight");
                    thread::sleep(Duration::from_millis(1500));
                },
                Err(e) => {
                    consecutive_errors += 1;
                    log_error(&format!("Error: {}", e));
                    let _ = self.tab.press_key("ArrowRight");
                    thread::sleep(Duration::from_millis(1500));
                }
            }

            if consecutive_errors > 8 {
                log_info("Too many consecutive errors. Exiting batch.");
                let _ = self.tab.press_key("Escape");
                break;
            }
        }

        log_info(&format!("Batch complete. Total saved: {}", story_count));
        Ok(())
    }

    async fn download_active_story(&self, username: &str, history: &mut HashSet<String>) -> Result<bool> {
        for _attempt in 1..=20 { // 10 seconds retry
            
            // A. PRECISE PAUSE (Unchanged as requested)
            let js_freeze = r#"
                (function() {
                    // Video Pause
                    let v = document.querySelector('video');
                    if (v && !v.paused && v.readyState > 2) { v.pause(); }
                    // Image Pause (UI Button)
                    let pauseBtn = document.querySelector('svg[aria-label="Pause"]');
                    if (pauseBtn) {
                        let btn = pauseBtn.closest('div[role="button"]') || pauseBtn.parentElement;
                        if (btn) btn.click();
                    }
                })()
            "#;
            let _ = self.tab.evaluate(js_freeze, false);

            // B. IDENTIFY MEDIA (Updated for Images)
            let js_identify = r#"
                (function() {
                    let urls = window.__intercepted_urls || [];
                    let candidates = [];
                    
                    // 1. NET (Backwards)
                    for (let i = urls.length - 1; i >= 0; i--) {
                        candidates.push("NET|" + urls[i]);
                    }
                    
                    // 2. DOM - VIDEO
                    let v = document.querySelector('video');
                    if (v && v.currentSrc && !v.currentSrc.startsWith('blob:')) candidates.push("DOM_VIDEO|" + v.currentSrc);

                    // 3. DOM - IMAGE (FIXED LOGIC)
                    // Instead of looking for specific attributes, we look for SIZE.
                    // The main story image is always the largest image on screen.
                    let images = Array.from(document.querySelectorAll('img'));
                    for (let img of images) {
                        // Must be visible and large (typical story is > 300px wide)
                        if (img.naturalWidth > 300 && img.src.includes('instagram')) {
                             candidates.push("DOM_IMAGE|" + img.src);
                             // If it also has srcset, grab the best quality
                             if (img.srcset) {
                                 let parts = img.srcset.split(',');
                                 candidates.push("DOM_IMAGE|" + parts[parts.length - 1].trim().split(' ')[0]);
                             }
                        }
                    }

                    return [...new Set(candidates)].join(';');
                })()
            "#;

            let raw_result = match self.tab.evaluate(js_identify, false) {
                Ok(res) => res.value.unwrap().as_str().unwrap_or("").to_string(),
                Err(_) => "".to_string(),
            };

            let items: Vec<&str> = raw_result.split(';').collect();
            let mut found_new = false;

            for item in items {
                if item.is_empty() { continue; }
                let parts: Vec<&str> = item.split('|').collect();
                if parts.len() < 2 { continue; }
                
                let source_type = parts[0]; 
                let mut url = parts[1].to_string();

                if url.len() < 15 { continue; }

                if url.contains(".mp4") {
                    if let Some(idx) = url.find("&bytestart") { url = url[..idx].to_string(); }
                    if let Some(idx) = url.find("?bytestart") { url = url[..idx].to_string(); }
                }

                if history.contains(&url) { continue; }

                // FOUND ONE!
                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                // Determine extension based on URL content
                let ext = if url.contains(".mp4") { "mp4" } else { "jpg" };
                let fname = format!("{}_{}.{}", username, timestamp, ext);

                log_info(&format!("Found new {} via {}! Downloading...", ext, source_type));

                let js_fetch = format!(r#"
                    (async function() {{
                        try {{
                            const response = await fetch("{}", {{ cache: 'force-cache' }});
                            const blob = await response.blob();
                            return await new Promise((resolve) => {{
                                const reader = new FileReader();
                                reader.onloadend = () => resolve(reader.result);
                                reader.readAsDataURL(blob);
                            }});
                        }} catch (err) {{ return "ERROR"; }}
                    }})()
                "#, url);

                match self.tab.evaluate(&js_fetch, true) {
                    Ok(res_fetch) => {
                        if let Some(data_val) = res_fetch.value {
                            let data_uri = data_val.as_str().unwrap_or("");
                            if data_uri.starts_with("data:") {
                                if let Ok(_) = save_base64_file(data_uri, &fname) {
                                    history.insert(url);
                                    found_new = true;
                                    break; 
                                }
                            }
                        }
                    },
                    Err(_) => {}
                }
            }

            if found_new {
                return Ok(true);
            }
            thread::sleep(Duration::from_millis(500));
        }
        Ok(false)
    }
}
