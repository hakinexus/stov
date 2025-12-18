use headless_chrome::{Browser, Tab, Element, protocol::cdp::Network::CookieParam};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
use std::collections::HashSet; 
use crate::config::*;
use crate::utils::{log_info, log_error, save_base64_file, save_screenshot, save_html, save_profile, mux_video_audio, rename_video_only};

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
        Ok(())
    }

    fn inject_sniffer(&self) {
        let script = r#"
            if (!window.__sniffer_active) {
                window.__intercepted_urls = [];
                const observer = new PerformanceObserver((list) => {
                    list.getEntries().forEach((entry) => {
                        // Capture MP4, M4A, JPG
                        if (entry.name.includes('.mp4') || entry.name.includes('.m4a') || (entry.name.includes('.jpg') && entry.name.includes('instagram'))) {
                            window.__intercepted_urls.push({
                                url: entry.name,
                                size: entry.transferSize || 0
                            });
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

    fn safely_click_login(&self) -> Result<()> {
        log_info("Activating Login...");
        if let Ok(buttons) = self.tab.find_elements("button") {
            for btn in buttons {
                if let Ok(text) = btn.get_inner_text() {
                    let clean_text = text.to_lowercase();
                    if clean_text.contains("show") { continue; }
                    if clean_text.contains("log in") {
                        let _ = btn.click();
                        return Ok(());
                    }
                }
            }
        }
        if let Ok(btn) = self.tab.find_element(SEL_SUBMIT) {
             let text = btn.get_inner_text().unwrap_or_default().to_lowercase();
             if !text.contains("show") {
                 let _ = btn.click();
                 return Ok(());
             }
        }
        let _ = self.tab.press_key("Enter");
        Ok(())
    }

    pub fn login_with_session(&self, session_id: &str) -> Result<()> {
        log_info("Attempting Login via Saved Session...");
        self.tab.navigate_to("https://www.instagram.com")?;
        
        let cookie = CookieParam {
            name: "sessionid".to_string(),
            value: session_id.to_string(),
            url: Some("https://www.instagram.com".to_string()),
            domain: Some(".instagram.com".to_string()),
            path: Some("/".to_string()),
            secure: Some(true),
            http_only: Some(true),
            same_site: None,
            expires: None,
            priority: None,
            source_scheme: None,
            source_port: None,
            partition_key: None,
            same_party: None,
        };
        
        self.tab.set_cookies(vec![cookie])?;
        log_info("Session cookie injected.");
        self.tab.reload(true, None)?;
        
        log_info("Verifying Session...");
        thread::sleep(Duration::from_secs(5));
        
        if self.tab.find_element(SEL_HOME_ICON).is_ok() || self.tab.find_element(SEL_AVATAR).is_ok() {
            log_info("Session Login Successful!");
            return Ok(());
        }
        if let Ok(el) = self.tab.find_element_by_xpath("//button[contains(text(), 'Not Now')]") {
             let _ = el.click();
             log_info("Session Login Successful (Popup dismissed).");
             return Ok(());
        }
        Err(anyhow!("Session Expired or Invalid. Please login manually."))
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
        thread::sleep(Duration::from_secs(2));

        for attempt in 1..=3 {
            if attempt > 1 { log_info(&format!("Retry attempt {}/3...", attempt)); }
            if let Err(e) = self.safely_click_login() { log_error(&format!("Click failed: {}", e)); }

            log_info("Verifying authentication...");
            let start_time = Instant::now();
            let mut retry_needed = false;

            while start_time.elapsed() < Duration::from_secs(20) {
                let mut success = false;
                if self.tab.find_element(SEL_HOME_ICON).is_ok() || self.tab.find_element(SEL_AVATAR).is_ok() { success = true; }
                if let Ok(el) = self.tab.find_element_by_xpath("//button[contains(text(), 'Not Now')]") { let _ = el.click(); success = true; }
                if !self.tab.get_url().contains("accounts/login") && !self.tab.get_url().contains("challenge") { success = true; }

                if success {
                    log_info("Login Verified.");
                    if let Ok(cookies) = self.tab.get_cookies() {
                        for c in cookies {
                            if c.name == "sessionid" {
                                let _ = save_profile(user, &c.value);
                                log_info("Profile saved.");
                                break;
                            }
                        }
                    }
                    self.snapshot(PROOF_DIR, "login_success");
                    return Ok(());
                }

                if let Ok(el) = self.tab.find_element("p[role='alert']") {
                    if let Ok(text) = el.get_inner_text() {
                        if text.to_lowercase().contains("incorrect") { return Err(anyhow!("Incorrect Password")); }
                        if text.to_lowercase().contains("problem") { retry_needed = true; break; }
                    }
                }
                thread::sleep(Duration::from_millis(500));
            }

            if retry_needed {
                log_info("Waiting 3s before retry...");
                thread::sleep(Duration::from_secs(3));
                continue; 
            } else if attempt == 3 {
                return Err(anyhow!("Login Timed Out"));
            }
        }
        Err(anyhow!("Login failed after retries"))
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

        // EXPERT: Do NOT clear logs here. We need the logs from the first video load.
        // self.clear_network_logs(); 

        loop {
            let current_url = self.tab.get_url();
            if !current_url.contains("stories") { log_info("Batch ended (Returned to feed)."); break; }
            if !current_url.contains(username) { log_info("Batch ended (Moved to different user)."); let _ = self.tab.press_key("Escape"); break; }

            match self.download_active_story(username, &mut downloaded_history).await {
                Ok(true) => {
                    story_count += 1;
                    consecutive_errors = 0;
                    log_info(&format!("Story #{} Saved.", story_count));
                    self.clear_network_logs(); // Clear logs ONLY after success
                    log_info("Moving to next...");
                    let _ = self.tab.press_key("ArrowRight");
                    thread::sleep(Duration::from_millis(1500));
                },
                Ok(false) => {
                    consecutive_errors += 1;
                    log_info("Skipping...");
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
                log_info("Too many errors. Exiting batch.");
                let _ = self.tab.press_key("Escape");
                break;
            }
        }
        log_info(&format!("Batch complete. Total saved: {}", story_count));
        Ok(())
    }

    async fn download_active_story(&self, username: &str, history: &mut HashSet<String>) -> Result<bool> {
        let mut audio_wait_counter = 0;

        for _attempt in 1..=25 { 
            
            // 1. FORCE AUDIO/VIDEO UNMUTE
            let js_freeze = r#"
                (function() {
                    let v = document.querySelector('video');
                    if (v) {
                        v.muted = false; // Trigger Audio Request
                        v.volume = 1.0;
                        if (!v.paused && v.readyState > 2) { v.pause(); }
                    }
                    // Handle Image Pause
                    let pauseBtn = document.querySelector('svg[aria-label="Pause"]');
                    if (pauseBtn) {
                        let btn = pauseBtn.closest('div[role="button"]') || pauseBtn.parentElement;
                        if (btn) btn.click();
                    }
                })()
            "#;
            let _ = self.tab.evaluate(js_freeze, false);

            // 2. IDENTIFY ASSETS
            let js_identify = r#"
                (function() {
                    let urls = window.__intercepted_urls || [];
                    let candidates = [];
                    // NET STREAMS
                    for (let entry of urls) {
                        let size = entry.size || 0;
                        if (entry.url.includes('.mp4') || entry.url.includes('.m4a')) {
                            candidates.push("NET_MP4|" + entry.url + "|" + size);
                        } else if (entry.url.includes('.jpg')) {
                            candidates.push("NET_JPG|" + entry.url + "|" + size);
                        }
                    }
                    // DOM IMAGE (Strict Vertical Check)
                    let images = Array.from(document.querySelectorAll('img'));
                    // Filter: Must be Vertical (Height > Width) AND Large (>300px)
                    // This ignores square profile pics and feed posts.
                    let target = images.find(i => i.naturalWidth > 300 && i.naturalHeight > i.naturalWidth && !i.alt.includes('profile'));
                    if (target) {
                         if (target.srcset) {
                             // Grab largest from srcset
                             let parts = target.srcset.split(',');
                             let best = parts.reduce((prev, curr) => {
                                 let wCurr = parseInt(curr.trim().split(' ')[1]) || 0;
                                 let wPrev = parseInt(prev.trim().split(' ')[1]) || 0;
                                 return wCurr > wPrev ? curr : prev;
                             });
                             candidates.push("DOM_IMAGE|" + best.trim().split(' ')[0] + "|0");
                         } else {
                             candidates.push("DOM_IMAGE|" + target.src + "|0");
                         }
                    }
                    return [...new Set(candidates)].join(';');
                })()
            "#;

            let raw_result = match self.tab.evaluate(js_identify, false) {
                Ok(res) => res.value.unwrap().as_str().unwrap_or("").to_string(),
                Err(_) => "".to_string(),
            };

            if raw_result.is_empty() {
                thread::sleep(Duration::from_millis(500));
                continue;
            }

            let items: Vec<&str> = raw_result.split(';').collect();
            let mut mp4_candidates: Vec<(String, usize)> = Vec::new();
            let mut image_url = String::new();
            
            for item in items {
                let parts: Vec<&str> = item.split('|').collect();
                if parts.len() < 3 { continue; }
                let kind = parts[0];
                let mut url = parts[1].to_string();
                let size: usize = parts[2].parse().unwrap_or(0);

                // Clean URL
                if url.contains(".mp4") {
                    if let Some(idx) = url.find("&bytestart") { url = url[..idx].to_string(); }
                    if let Some(idx) = url.find("?bytestart") { url = url[..idx].to_string(); }
                }

                if history.contains(&url) { continue; }

                if kind == "DOM_IMAGE" { image_url = url; } 
                else if kind == "NET_MP4" { mp4_candidates.push((url, size)); }
            }

            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let base_filename = format!("{}_{}", username, timestamp);

            // --- EXECUTION PRIORITY ---

            // Priority 1: Video + Audio Muxing
            // We accumulate candidates. If we have > 0 candidates, we assume video logic.
            if !mp4_candidates.is_empty() {
                // Sort by size (Largest is Video, Smaller is Audio)
                mp4_candidates.sort_by(|a, b| b.1.cmp(&a.1));
                
                let video_url = mp4_candidates[0].0.clone();
                let mut audio_url = String::new();

                // Find a smaller file that isn't the video (Audio)
                for (url, size) in &mp4_candidates {
                    if *url != video_url && *size > 0 && *size < 800_000 {
                        audio_url = url.clone();
                        break;
                    }
                }

                // CONVERGENCE LOCK:
                // If we have video but NO audio, wait up to 4 seconds (8 ticks) for audio to appear
                if !video_url.is_empty() && audio_url.is_empty() && audio_wait_counter < 8 {
                    log_info("Found Video, waiting for Audio stream...");
                    audio_wait_counter += 1;
                    thread::sleep(Duration::from_millis(500));
                    continue; 
                }

                if !video_url.is_empty() {
                    let v_name = format!("{}_v.mp4", base_filename);
                    let a_name = format!("{}_audio.mp4", base_filename);
                    let final_name = format!("{}.mp4", base_filename);

                    let v_ok = self.fetch_and_save(&video_url, &v_name).await.is_ok();
                    let mut a_ok = false;
                    
                    if !audio_url.is_empty() {
                        a_ok = self.fetch_and_save(&audio_url, &a_name).await.is_ok();
                    }

                    if v_ok && a_ok {
                        // Flawless Victory: Mux
                        if let Ok(_) = mux_video_audio(&v_name, &a_name, &final_name) {
                            history.insert(video_url);
                            history.insert(audio_url);
                            return Ok(true);
                        }
                    } else if v_ok {
                        // Fallback: Muted Video
                        log_info("Audio stream unavailable. Saving Video only.");
                        rename_video_only(&v_name, &final_name)?;
                        history.insert(video_url);
                        return Ok(true);
                    }
                }
            }

            // Priority 2: Image
            // Only if NO video candidates were found
            if !image_url.is_empty() && mp4_candidates.is_empty() {
                log_info("Found Image! Downloading...");
                let fname = format!("{}.jpg", base_filename);
                if self.fetch_and_save(&image_url, &fname).await.is_ok() {
                    history.insert(image_url);
                    return Ok(true);
                }
            }

            thread::sleep(Duration::from_millis(500));
        }
        Ok(false)
    }

    async fn fetch_and_save(&self, url: &str, filename: &str) -> Result<()> {
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
            Ok(res) => {
                if let Some(val) = res.value {
                    let data = val.as_str().unwrap_or("");
                    if data.starts_with("data:") {
                        return save_base64_file(data, filename);
                    }
                }
            },
            Err(_) => {}
        }
        Err(anyhow!("Fetch failed"))
    }
}
