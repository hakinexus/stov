use headless_chrome::{Browser, Tab, Element, protocol::cdp::Network::CookieParam};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use std::thread;
use rand::Rng;
use anyhow::{Result, anyhow};
use std::collections::HashSet; 
use crate::config::*;
use crate::utils::{log_info, log_error, save_base64_file, save_screenshot, save_html, save_profile, mux_video_audio, normalize_video};

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
                window.__intercepted_urls = new Array();
                // We use a high-res timestamp from when we inject to filter old events
                window.__bot_start_time = performance.now();
                
                const observer = new PerformanceObserver((list) => {
                    list.getEntries().forEach((entry) => {
                        if (entry.name.includes('.mp4') || entry.name.includes('.m4a') || (entry.name.includes('.jpg') && entry.name.includes('instagram'))) {
                            window.__intercepted_urls.push({
                                url: entry.name,
                                size: Math.max(entry.transferSize || 0, entry.decodedBodySize || 0),
                                time: entry.startTime || performance.now()
                            });
                            // Keep buffer lean
                            if (window.__intercepted_urls.length > 500) window.__intercepted_urls.shift();
                        }
                    });
                });
                observer.observe({ entryTypes: new Array('resource') });
                window.__sniffer_active = true;
            }
        "#;
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
        
        let mut cookie_vec = Vec::new();
        cookie_vec.push(cookie);
        self.tab.set_cookies(cookie_vec)?;
        
        log_info("Session cookie injected.");
        self.tab.reload(true, None)?;
        
        log_info("Verifying Session...");
        thread::sleep(Duration::from_secs(5));
        
        if self.tab.find_element(SEL_HOME_ICON).is_ok() || self.tab.find_element(SEL_AVATAR).is_ok() {
            log_info("Session Login Successful!");
            return Ok(());
        }
        
        if let Ok(el) = self.tab.find_element_by_xpath("//button\x5Bcontains(text(), 'Not Now')\x5D") {
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

        let mut cookie_xpaths = Vec::new();
        cookie_xpaths.push("//button\x5Bcontains(text(), 'Allow all cookies')\x5D");
        cookie_xpaths.push("//button\x5Bcontains(text(), 'Allow')\x5D");
        cookie_xpaths.push("//button\x5Bcontains(text(), 'Decline')\x5D");
        
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
                if let Ok(el) = self.tab.find_element_by_xpath("//button\x5Bcontains(text(), 'Not Now')\x5D") { let _ = el.click(); success = true; }
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

                if let Ok(el) = self.tab.find_element("p\x5Brole='alert'\x5D") {
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

    // --- GOD-LEVEL FIX: HYBRID ID EXTRACTION ---
    // Returns Some("HEAD") if it's the base story URL, or the Numeric ID if available.
    fn get_story_id(&self, url: &str) -> Option<String> {
        let parts: Vec<&str> = url.split('/').collect();
        // Check for numeric ID first
        for p in parts.iter().rev() {
            if !p.is_empty() && p.chars().all(char::is_numeric) && p.len() > 5 {
                return Some(p.to_string());
            }
        }
        // If no ID, but we are in stories, return HEAD to allow 1st story download
        if url.contains("/stories/") {
            return Some("HEAD".to_string());
        }
        None
    }

    // --- RESTORED & UPGRADED: Explicit Modal Destroyer ---
    fn handle_view_story_modal(&self) {
        let js = r#"
            (function() {
                let clicked = false;
                // Specific "View Story" Button
                let buttons = Array.from(document.querySelectorAll('button'));
                for (let btn of buttons) {
                    if (btn.innerText.includes('View story')) {
                        btn.click();
                        clicked = true;
                        break;
                    }
                }
                // Fallback: "See" or "View" generic text in center div
                if (!clicked) {
                    let divs = Array.from(document.querySelectorAll('div[role="button"]'));
                    for (let d of divs) {
                        if (d.innerText.includes('View') || d.innerText.includes('See')) {
                            d.click();
                            break;
                        }
                    }
                }
            })()
        "#;
        let _ = self.tab.evaluate(js, false);
    }

    pub async fn process_targets(&self, targets: Vec<String>) -> Result<()> {
        for target in targets {
            log_info(&format!("Checking target: {}", target));
            
            // Step 1: Navigate to profile
            let profile_url = format!("https://www.instagram.com/{}/", target);
            if let Err(_) = self.tab.navigate_to(&profile_url) { continue; }
            thread::sleep(Duration::from_secs(3)); 

            if self.tab.find_element(SEL_STORY_RING).is_ok() {
                log_info("Story found! Preparing isolated environment...");
                
                // Clear logs BEFORE navigation to kill the Feed reels from memory
                let _ = self.tab.evaluate("performance.clearResourceTimings(); window.__intercepted_urls = new Array();", false);
                
                // PURE ISOLATION
                let story_url = format!("https://www.instagram.com/stories/{}/", target);
                let _ = self.tab.navigate_to(&story_url);
                
                // Wait for navigation
                thread::sleep(Duration::from_secs(4));

                // SECONDARY PURGE
                let _ = self.tab.evaluate("performance.clearResourceTimings(); window.__intercepted_urls = new Array();", false);

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
        self.handle_view_story_modal();
        
        // Wait for ANY valid ID (Numeric or HEAD)
        let mut loaded = false;
        for _ in 0..20 {
            self.handle_view_story_modal();
            let u = self.tab.get_url();
            if self.get_story_id(&u).is_some() {
                loaded = true;
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }

        if !loaded {
            log_error("Failed to resolve Story ID. Viewer did not load.");
            return Ok(()); 
        }

        let mut processed_ids: HashSet<String> = HashSet::new();
        let mut url_history: HashSet<String> = HashSet::new();
        let mut story_count = 0;
        let mut consecutive_errors = 0;

        loop {
            let current_url = self.tab.get_url();
            
            if !current_url.contains(&format!("stories/{}", username)) { 
                log_info("Batch ended (Viewer closed automatically)."); 
                break; 
            }

            let current_id_opt = self.get_story_id(&current_url);
            
            if current_id_opt.is_none() {
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            
            let current_id = current_id_opt.unwrap();

            // ID LOCK: Only process if ID is new
            if processed_ids.contains(&current_id) {
                thread::sleep(Duration::from_millis(500));
            } else {
                // Pass url_history to track file segments
                match self.download_active_story(username, &current_id, &mut url_history).await {
                    Ok(true) => {
                        story_count += 1;
                        consecutive_errors = 0;
                        log_info(&format!("Story #{} Saved.", story_count));
                        processed_ids.insert(current_id.clone()); 
                    },
                    Ok(false) => {
                        consecutive_errors += 1;
                        log_info("Skipping/Timeout...");
                        processed_ids.insert(current_id.clone());
                    },
                    Err(e) => {
                        consecutive_errors += 1;
                        log_error(&format!("Error: {}", e));
                        processed_ids.insert(current_id.clone());
                    }
                }
                
                // Clear logs for next story
                let _ = self.tab.evaluate("window.__intercepted_urls = new Array(); performance.clearResourceTimings();", false);

                log_info("Moving to next...");
                let _ = self.tab.press_key("ArrowRight");
            }

            if consecutive_errors > 6 {
                log_info("Too many errors. Exiting batch.");
                break;
            }
            
            // Sync Wait for ID Change
            let prev_id = current_id.clone();
            let mut id_changed = false;
            for _ in 0..20 {
                self.handle_view_story_modal();
                thread::sleep(Duration::from_millis(300));
                let new_url = self.tab.get_url();
                let new_id_opt = self.get_story_id(&new_url);
                
                if let Some(nid) = new_id_opt {
                    if nid != prev_id {
                        id_changed = true;
                        break;
                    }
                } else if !new_url.contains("stories") {
                    id_changed = true;
                    break;
                }
            }
            
            if !id_changed {
                log_info("End of stories reached or transition timed out.");
                break;
            }
            
            thread::sleep(Duration::from_millis(1500));
        }
        
        log_info(&format!("Batch complete. Total saved: {}", story_count));
        Ok(())
    }

    async fn download_active_story(&self, username: &str, current_id: &str, history: &mut HashSet<String>) -> Result<bool> {
        let mut failed_urls: HashSet<String> = HashSet::new();

        // 30 Seconds timeout per story
        for _attempt in 1..=60 { 
            self.handle_view_story_modal();

            // 1. FREEZE MEDIA
            let js_freeze = r#"
                (function() {
                    let videos = document.querySelectorAll('video');
                    videos.forEach(v => {
                        v.muted = false;
                        v.volume = 1.0;
                        if (v.paused && v.readyState >= 2) { v.pause(); }
                        if (v.readyState < 2) { v.play().catch(e=>{}); }
                    });
                })()
            "#;
            let _ = self.tab.evaluate(js_freeze, false);

            let history_json = serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string());

            // 2. THE GOD-LEVEL FILTER
            let js_identify = format!(r#"
                (function() {{
                    function getCleanUrl(urlStr) {{
                        try {{
                            let u = new URL(urlStr);
                            u.searchParams.delete('bytestart');
                            u.searchParams.delete('bytestop');
                            return u.toString();
                        }} catch(e) {{ return urlStr; }}
                    }}

                    let downloaded = new Set({});
                    let centerX = window.innerWidth / 2;
                    let centerY = window.innerHeight / 2;
                    
                    let raw_resources = new Array();
                    if (window.__intercepted_urls) {{
                        window.__intercepted_urls.forEach(u => raw_resources.push(u));
                    }}
                    let perf = performance.getEntriesByType('resource');
                    perf.forEach(p => raw_resources.push(p));
                    
                    let resources = new Array();
                    let seen = new Set();
                    let botStart = window.__bot_start_time || 0;

                    raw_resources.forEach(r => {{
                        let u = r.name || r.url;
                        let t = r.startTime || r.responseEnd || 0;
                        
                        if (t > botStart && !seen.has(u)) {{
                            seen.add(u);
                            let clean = getCleanUrl(u);
                            if (!downloaded.has(clean)) {{
                                resources.push({{
                                    url: u,
                                    cleanUrl: clean,
                                    size: Math.max(r.transferSize || 0, r.decodedBodySize || 0, r.size || 0),
                                    time: t
                                }});
                            }}
                        }}
                    }});
                    
                    resources.sort((a,b) => b.time - a.time);

                    let allVideos = Array.from(document.querySelectorAll('video'));
                    let bestVideo = null;
                    
                    for (let v of allVideos) {{
                        let rect = v.getBoundingClientRect();
                        if (centerX >= rect.left && centerX <= rect.right && centerY >= rect.top && centerY <= rect.bottom) {{
                            bestVideo = v; break; 
                        }}
                    }}

                    if (bestVideo) {{
                        let src = bestVideo.src;
                        if (!src) {{
                            let srcTag = bestVideo.querySelector('source');
                            if (srcTag) src = srcTag.src;
                        }}
                        
                        if (src && src.startsWith('blob:')) {{
                            // THE FIREWALL: Must NOT contain audio tags.
                            let vids = resources.filter(r => {{
                                let uLow = r.url.toLowerCase();
                                let isAudio = uLow.includes('mime=audio') || uLow.includes('audio%2f') || uLow.includes('.m4a');
                                let isVideo = uLow.includes('.mp4') || uLow.includes('mime=video') || uLow.includes('bytestart');
                                return isVideo && !isAudio && r.size > 100000;
                            }});
                            
                            if (vids.length > 0) return "DOM_VIDEO|" + vids[0].cleanUrl;
                        }} else if (src) {{
                            return "DOM_VIDEO|" + getCleanUrl(src);
                        }}
                    }}

                    let allImages = Array.from(document.querySelectorAll('img'));
                    for (let img of allImages) {{
                        let rect = img.getBoundingClientRect();
                        if (centerX >= rect.left && centerX <= rect.right && centerY >= rect.top && centerY <= rect.bottom) {{
                            if (rect.width > 200) {{
                                if (img.srcset) {{
                                    let parts = img.srcset.split(',');
                                    let best = parts.pop(); 
                                    return "DOM_IMAGE|" + getCleanUrl(best.trim().split(' ').shift());
                                }}
                                return "DOM_IMAGE|" + getCleanUrl(img.src);
                            }}
                        }}
                    }}

                    return "EMPTY";
                }})()
            "#, history_json);

            let raw_result = match self.tab.evaluate(&js_identify, false) {
                Ok(res) => res.value.and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
                Err(_) => "".to_string(),
            };

            if raw_result == "EMPTY" || raw_result.is_empty() {
                thread::sleep(Duration::from_millis(500));
                continue;
            }

            let parts: Vec<&str> = raw_result.split('|').collect();
            let kind = *parts.get(0).unwrap_or(&"");
            let url = parts.get(1).unwrap_or(&"").to_string();

            if failed_urls.contains(&url) { 
                thread::sleep(Duration::from_millis(500));
                continue; 
            }

            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            // Use unique ID in filename
            let base_filename = format!("{}_{}_{}", username, timestamp, current_id);

            // 3. EXECUTION
            if kind == "DOM_VIDEO" {
                let v_name = format!("{}_v.mp4", base_filename);
                
                if self.fetch_and_save(&url, &v_name).await.is_ok() {
                    
                    let js_find_audio = format!(r#"
                        (function() {{
                            let resources = performance.getEntriesByType('resource');
                            let audios = new Array();
                            resources.forEach(r => {{
                                let u = r.name || r.url;
                                let uLow = u.toLowerCase();
                                if ((uLow.includes('mime=audio') || uLow.includes('audio%2f') || uLow.includes('.m4a')) && r.transferSize > 5000) {{
                                    audios.push({{url: u, time: r.startTime}});
                                }}
                            }});
                            audios.sort((a,b) => b.time - a.time);
                            
                            if (audios.length > 0) {{
                                let best = audios.shift();
                                try {{
                                    let uObj = new URL(best.url);
                                    uObj.searchParams.delete('bytestart');
                                    uObj.searchParams.delete('bytestop');
                                    return uObj.toString();
                                }} catch(e) {{ return best.url; }}
                            }}
                            return "";
                        }})()
                    "#);
                    
                    let audio_url = match self.tab.evaluate(&js_find_audio, false) {
                        Ok(res) => res.value.and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
                        Err(_) => "".to_string(),
                    };

                    let final_name = format!("{}.mp4", base_filename);

                    if !audio_url.is_empty() && audio_url != url {
                        let a_name = format!("{}_audio.mp4", base_filename);
                        if self.fetch_and_save(&audio_url, &a_name).await.is_ok() {
                            let _ = mux_video_audio(&v_name, &a_name, &final_name);
                            history.insert(url.clone());
                            history.insert(audio_url);
                            return Ok(true);
                        }
                    }
                    
                    log_info("No separate audio stream found. Normalizing base video.");
                    let _ = normalize_video(&v_name, &final_name);
                    history.insert(url.clone());
                    return Ok(true);

                } else {
                    failed_urls.insert(url);
                }
            } else if kind == "DOM_IMAGE" {
                let fname = format!("{}.jpg", base_filename);
                if self.fetch_and_save(&url, &fname).await.is_ok() {
                    history.insert(url);
                    return Ok(true);
                } else {
                    failed_urls.insert(url);
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
                    const response = await fetch("{}", {{ credentials: 'omit', cache: 'no-store' }});
                    if (!response.ok) return "ERROR_HTTP_" + response.status;
                    const blob = await response.blob();
                    return await new Promise((resolve) => {{
                        const reader = new FileReader();
                        reader.onloadend = () => resolve(reader.result);
                        reader.readAsDataURL(blob);
                    }});
                }} catch (err) {{ return "ERROR_CATCH"; }}
            }})()
        "#, url);

        match self.tab.evaluate(&js_fetch, true) {
            Ok(res) => {
                if let Some(val) = res.value {
                    let data = val.as_str().unwrap_or("");
                    if data.starts_with("data:") {
                        return save_base64_file(data, filename);
                    } else {
                        return Err(anyhow!("Fetch returned error: {}", data));
                    }
                }
            },
            Err(e) => return Err(anyhow!("Evaluate failed: {}", e))
        }
        Err(anyhow!("Fetch failed entirely"))
    }
}
