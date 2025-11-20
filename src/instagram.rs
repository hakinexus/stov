use headless_chrome::{Browser, Tab, Element};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
// We need HashSet to track downloaded URLs to avoid duplicates
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

    // --- SNIFFER SETUP ---
    fn inject_sniffer(&self) {
        // We use PerformanceObserver to catch media requests in real-time
        let script = r#"
            window.__intercepted_urls = [];
            const observer = new PerformanceObserver((list) => {
                list.getEntries().forEach((entry) => {
                    // Capture MP4s and High-Res JPGs
                    if (entry.name.includes('.mp4') || (entry.name.includes('.jpg') && entry.name.includes('instagram'))) {
                        window.__intercepted_urls.push(entry.name);
                    }
                });
            });
            observer.observe({ entryTypes: ['resource'] });
        "#;
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
                // Call the new Batch Processor
                let _ = self.process_story_batch(&target).await;
            } else {
                log_info("No stories found for this user.");
            }
            
            thread::sleep(Duration::from_secs(rand::thread_rng().gen_range(3..6)));
        }
        Ok(())
    }

    // --- NEW: BATCH STORY PROCESSOR ---
    async fn process_story_batch(&self, username: &str) -> Result<()> {
        self.inject_sniffer();
        
        // Open the Story Viewer
        if let Ok(el) = self.tab.find_element(SEL_STORY_RING) { let _ = el.click(); }
        
        thread::sleep(Duration::from_secs(3)); // Allow player to load

        let mut downloaded_urls: HashSet<String> = HashSet::new();
        let mut story_count = 0;
        let mut empty_cycles = 0; // To detect if we are stuck on an ad or loading

        log_info(&format!("Starting batch extraction for: {}", username));

        // LOOP: Continues until we exit the user's story feed
        loop {
            // 1. Check where we are
            let current_url = self.tab.get_url();
            if !current_url.contains("stories") {
                log_info("Exited story viewer (Back to Feed). Batch complete.");
                break;
            }
            // Expert Check: Did we drift to the next user?
            if !current_url.contains(username) {
                log_info("Moved to a different user's story. Stopping batch.");
                // We press escape to close the viewer so we can return to main logic cleanly
                let _ = self.tab.press_key("Escape");
                break;
            }

            // 2. Clear old logs to keep sniffing fresh
            let _ = self.tab.evaluate("performance.clearResourceTimings(); window.__intercepted_urls = [];", false);
            
            // Wait a moment for media to buffer/play
            thread::sleep(Duration::from_millis(2500));

            // 3. Scrape Media URL
            let found_media = self.attempt_download(username, &mut downloaded_urls).await;

            if found_media {
                story_count += 1;
                empty_cycles = 0;
                log_info(&format!("Story #{} saved. Moving to next...", story_count));
            } else {
                empty_cycles += 1;
                log_info("No new media found (Ad or Buffering). Skipping...");
            }

            // 4. Navigation: Click "Next" (Right Arrow)
            // This is how we advance to the next story in the batch
            let _ = self.tab.press_key("ArrowRight");

            // 5. Safety Break: If we click next 5 times and find nothing, we assume we are stuck/done.
            if empty_cycles > 5 {
                log_info("Too many empty cycles. Assuming end of stories.");
                let _ = self.tab.press_key("Escape");
                break;
            }
        }

        log_info(&format!("Batch finished. Total stories saved: {}", story_count));
        Ok(())
    }

    // Helper function to handle the download logic for a single frame
    async fn attempt_download(&self, username: &str, downloaded_history: &mut HashSet<String>) -> bool {
        let js_identify = r#"
            (function() {
                // Combine Intercepted URLs + DOM Fallbacks
                let urls = window.__intercepted_urls || [];
                
                // DOM Video
                let v = document.querySelector('video');
                if (v && v.src && v.src.startsWith('http')) urls.push(v.src);
                let vsrc = document.querySelector('video source');
                if (vsrc && vsrc.src && vsrc.src.startsWith('http')) urls.push(vsrc.src);

                // DOM Image
                let imgs = Array.from(document.querySelectorAll('img'));
                // Find the big main image (usually has srcset)
                let target = imgs.find(i => i.srcset && i.src.includes('instagram'));
                if (target) {
                     let parts = target.srcset.split(',');
                     urls.push(parts[parts.length - 1].trim().split(' ')[0]);
                }
                
                return [...new Set(urls)].join(';');
            })()
        "#;

        if let Ok(res) = self.tab.evaluate(js_identify, false) {
            if let Some(val) = res.value {
                let s = val.as_str().unwrap_or("");
                let candidates: Vec<&str> = s.split(';').collect();

                for c in candidates {
                    if c.len() < 15 { continue; }
                    
                    // Clean URL (remove range params)
                    let mut clean_url = c.to_string();
                    if let Some(idx) = clean_url.find("&bytestart") { clean_url = clean_url[..idx].to_string(); }
                    if let Some(idx) = clean_url.find("?bytestart") { clean_url = clean_url[..idx].to_string(); }

                    // DEDUPLICATION: Check if we already grabbed this file
                    if downloaded_history.contains(&clean_url) {
                        continue; 
                    }

                    // We found a NEW valid URL
                    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    let ext = if clean_url.contains(".mp4") { "mp4" } else { "jpg" };
                    let fname = format!("{}_{}.{}", username, timestamp, ext);

                    log_info(&format!("Fetching new media... ({})", ext));

                    // Browser-Side Fetch
                    let js_fetch = format!(r#"
                        (async function() {{
                            try {{
                                const response = await fetch("{}", {{ cache: 'no-store' }});
                                const blob = await response.blob();
                                return await new Promise((resolve) => {{
                                    const reader = new FileReader();
                                    reader.onloadend = () => resolve(reader.result);
                                    reader.readAsDataURL(blob);
                                }});
                            }} catch (err) {{ return "ERROR"; }}
                        }})()
                    "#, clean_url);

                    if let Ok(res_fetch) = self.tab.evaluate(&js_fetch, true) {
                        if let Some(data_val) = res_fetch.value {
                            let data_uri = data_val.as_str().unwrap_or("");
                            if data_uri.starts_with("data:") {
                                if let Ok(_) = save_base64_file(data_uri, &fname) {
                                    // Mark as downloaded
                                    downloaded_history.insert(clean_url);
                                    return true; // Success
                                }
                            }
                        }
                    }
                }
            }
        }
        return false; // No new media found this cycle
    }
}
