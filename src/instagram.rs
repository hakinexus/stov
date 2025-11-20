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
            window.__intercepted_urls = [];
            const observer = new PerformanceObserver((list) => {
                list.getEntries().forEach((entry) => {
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
                let _ = self.process_story_batch(&target).await;
            } else {
                log_info("No stories found for this user.");
            }
            thread::sleep(Duration::from_secs(rand::thread_rng().gen_range(3..6)));
        }
        Ok(())
    }

    // --- EXPERT BATCH PROCESSOR (Freeze & Fetch) ---
    async fn process_story_batch(&self, username: &str) -> Result<()> {
        self.inject_sniffer();
        
        // Open Story
        if let Ok(el) = self.tab.find_element(SEL_STORY_RING) { let _ = el.click(); }
        
        // Initial buffer wait
        thread::sleep(Duration::from_secs(3));

        let mut downloaded_history: HashSet<String> = HashSet::new();
        let mut story_count = 0;
        let mut consecutive_errors = 0;

        log_info(&format!("Starting batch extraction for: {}", username));

        loop {
            // 1. Safety Checks
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

            // 2. DOWNLOAD LOGIC
            // We pass the history so we don't download the same file twice
            match self.download_active_story(username, &mut downloaded_history).await {
                Ok(true) => {
                    story_count += 1;
                    consecutive_errors = 0;
                    log_info(&format!("Story #{} Saved. Moving to next...", story_count));
                },
                Ok(false) => {
                    // Returned false means duplicate or ad (skipped)
                    consecutive_errors = 0; 
                    log_info("Skipping (Duplicate/Ad)...");
                },
                Err(e) => {
                    consecutive_errors += 1;
                    log_error(&format!("Error on current story: {}", e));
                }
            }

            // 3. NAVIGATION (Crucial Step)
            // We press 'ArrowRight' to go next.
            let _ = self.tab.press_key("ArrowRight");

            // 4. TRANSITION WAIT
            // We wait for the URL or the Media Source to change.
            // This prevents the bot from firing on the same slide before it transitions.
            thread::sleep(Duration::from_millis(1500)); 

            if consecutive_errors > 5 {
                log_info("Too many errors. Exiting batch.");
                let _ = self.tab.press_key("Escape");
                break;
            }
        }

        log_info(&format!("Batch complete. Total saved: {}", story_count));
        Ok(())
    }

    // This function handles the "Freeze -> Extract -> Download" logic for a SINGLE story slide
    async fn download_active_story(&self, username: &str, history: &mut HashSet<String>) -> Result<bool> {
        
        // STEP A: PAUSE THE VIDEO (The "Freeze")
        // We inject JS to find the video element and force pause it.
        // This stops Instagram from skipping to the next story while we are downloading.
        let js_freeze = r#"
            (function() {
                let v = document.querySelector('video');
                if (v) { 
                    v.pause(); 
                    return "PAUSED";
                }
                return "IMAGE"; // Images don't need pausing
            })()
        "#;
        let _ = self.tab.evaluate(js_freeze, false);

        // Give a tiny moment for the pause to take effect and network logs to settle
        thread::sleep(Duration::from_millis(500));

        // STEP B: IDENTIFY THE MEDIA URL
        // We look for the High Res URL in DOM or Network Logs
        let js_identify = r#"
            (function() {
                let candidates = [];
                
                // 1. Network Log (Best for Videos)
                let resources = performance.getEntriesByType('resource');
                // Check last 50 requests
                for (let i = resources.length - 1; i >= Math.max(0, resources.length - 50); i--) {
                    let name = resources[i].name;
                    if (name.includes('.mp4') && !name.startsWith('blob:')) candidates.push(name);
                }

                // 2. DOM (Best for Images & Fallback Video)
                let v = document.querySelector('video');
                if (v && v.src && v.src.startsWith('http')) candidates.push(v.src);
                
                let vsrc = document.querySelector('video source');
                if (vsrc && vsrc.src && vsrc.src.startsWith('http')) candidates.push(vsrc.src);

                let imgs = Array.from(document.querySelectorAll('img'));
                let target = imgs.find(i => i.srcset && i.src.includes('instagram'));
                if (target) {
                     let parts = target.srcset.split(',');
                     candidates.push(parts[parts.length - 1].trim().split(' ')[0]);
                }
                
                // Return unique
                return [...new Set(candidates)].join(';');
            })()
        "#;

        if let Ok(res) = self.tab.evaluate(js_identify, false) {
            if let Some(val) = res.value {
                let s = val.as_str().unwrap_or("");
                let candidates: Vec<&str> = s.split(';').collect();

                for url in candidates {
                    if url.len() < 15 { continue; }

                    // Clean URL (Strip range for full video)
                    let mut clean_url = url.to_string();
                    if let Some(idx) = clean_url.find("&bytestart") { clean_url = clean_url[..idx].to_string(); }
                    if let Some(idx) = clean_url.find("?bytestart") { clean_url = clean_url[..idx].to_string(); }

                    // CHECK HISTORY (Deduplication)
                    if history.contains(&clean_url) {
                        continue; // Already got this one
                    }

                    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                    let ext = if clean_url.contains(".mp4") { "mp4" } else { "jpg" };
                    let fname = format!("{}_{}.{}", username, timestamp, ext);

                    // STEP C: BROWSER FETCH (Secure Download)
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
                    "#, clean_url);

                    match self.tab.evaluate(&js_fetch, true) {
                        Ok(res_fetch) => {
                            if let Some(data_val) = res_fetch.value {
                                let data_uri = data_val.as_str().unwrap_or("");
                                if data_uri.starts_with("data:") {
                                    // Save it
                                    if let Ok(_) = save_base64_file(data_uri, &fname) {
                                        history.insert(clean_url); // Add to history
                                        return Ok(true);
                                    }
                                }
                            }
                        },
                        Err(_) => {}
                    }
                }
            }
        }

        // If we reach here, we found no NEW media on this slide. 
        // It might be an Ad or a repeat. Return false so the loop skips it.
        Ok(false)
    }
}
