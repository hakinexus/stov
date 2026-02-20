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
                window.__intercepted_urls = new Array();
                const observer = new PerformanceObserver((list) => {
                    list.getEntries().forEach((entry) => {
                        if (entry.name.includes('.mp4') || entry.name.includes('.m4a') || (entry.name.includes('.jpg') && entry.name.includes('instagram'))) {
                            window.__intercepted_urls.push({
                                url: entry.name,
                                size: entry.transferSize || entry.decodedBodySize || 0,
                                time: entry.startTime || performance.now()
                            });
                            // Keep buffer reasonably sized
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

    pub async fn process_targets(&self, targets: Vec<String>) -> Result<()> {
        for target in targets {
            log_info(&format!("Checking target: {}", target));
            let url = format!("https://www.instagram.com/{}/", target);
            if let Err(_) = self.tab.navigate_to(&url) { continue; }
            thread::sleep(Duration::from_secs(5)); 

            // DESKTOP: Check specifically for the Header Canvas
            if self.tab.find_element(SEL_STORY_RING).is_ok() {
                log_info("Story found! Transitioning to viewer...");
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
        
        let mut transitioned = false;

        if let Ok(el) = self.tab.find_element(SEL_STORY_RING) { 
            let _ = el.click(); 
        }

        for _ in 0..10 {
            if self.tab.get_url().contains("stories") {
                transitioned = true;
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }

        if !transitioned {
            log_info("Canvas click missed. Trying fallback click on Profile Image...");
            if let Ok(el) = self.tab.find_element(SEL_PROFILE_IMG) {
                let _ = el.click();
            }
            for _ in 0..10 {
                if self.tab.get_url().contains("stories") {
                    transitioned = true;
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            }
        }

        if !transitioned {
            log_error("Failed to open Story Viewer. Aspect ratio or Layout mismatch.");
            return Ok(()); 
        }

        let mut downloaded_history: HashSet<String> = HashSet::new();
        let mut story_count = 0;
        let mut consecutive_errors = 0;

        loop {
            let current_url = self.tab.get_url();
            if !current_url.contains("stories") { log_info("Batch ended (Returned to feed)."); break; }
            if !current_url.contains(username) { log_info("Batch ended (Moved to different user)."); let _ = self.tab.press_key("Escape"); break; }

            match self.download_active_story(username, &mut downloaded_history).await {
                Ok(true) => {
                    story_count += 1;
                    consecutive_errors = 0;
                    log_info(&format!("Story #{} Saved.", story_count));
                    log_info("Moving to next...");
                    let _ = self.tab.press_key("ArrowRight");
                    thread::sleep(Duration::from_millis(1500));
                },
                Ok(false) => {
                    consecutive_errors += 1;
                    log_info("Skipping/Timeout...");
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
        let mut failed_urls: HashSet<String> = HashSet::new();

        for _attempt in 1..=25 { 
            
            // 1. FORCE AUDIO/VIDEO UNMUTE AND FREEZE
            let js_freeze = r#"
                (function() {
                    let videos = document.querySelectorAll('video');
                    videos.forEach(v => {
                        v.muted = false;
                        v.volume = 1.0;
                        if (!v.paused && v.readyState > 2) { v.pause(); }
                    });
                    
                    let svgs = Array.from(document.querySelectorAll('svg'));
                    let pauseBtn = svgs.find(s => s.getAttribute('aria-label') === 'Pause');
                    if (pauseBtn) {
                        let btn = pauseBtn.closest('button') || pauseBtn.closest('div') || pauseBtn.parentElement;
                        if (btn) btn.click();
                    }
                })()
            "#;
            let _ = self.tab.evaluate(js_freeze, false);

            // 2. GEOSPATIAL GEOMETRY CHECK (God-Level Fix for "Wrong Video")
            let js_identify = r#"
                (function() {
                    let centerX = window.innerWidth / 2;
                    let centerY = window.innerHeight / 2;
                    
                    let raw_resources = [
                        ...(window.__intercepted_urls || []), 
                        ...performance.getEntriesByType('resource')
                    ];
                    
                    let resources = [];
                    let seen = new Set();
                    raw_resources.forEach(r => {
                        let u = r.name || r.url;
                        if (!seen.has(u)) {
                            seen.add(u);
                            resources.push({
                                url: u,
                                size: r.transferSize || r.decodedBodySize || r.size || 0,
                                time: r.startTime || r.responseEnd || performance.now()
                            });
                        }
                    });
                    // Sort by TIME (Newest first) - critical for catching the 'current' story loading
                    resources.sort((a, b) => b.time - a.time);

                    // --- VIDEO FINDER ---
                    let allVideos = Array.from(document.querySelectorAll('video'));
                    let bestVideo = null;
                    
                    for (let v of allVideos) {
                        let rect = v.getBoundingClientRect();
                        // 1. MUST contain center point
                        if (centerX >= rect.left && centerX <= rect.right && centerY >= rect.top && centerY <= rect.bottom) {
                            
                            // 2. HEIGHT CHECK: Stories are tall. Posts are usually squares.
                            // If video height is > 65% of viewport, it's definitely a Story.
                            // Feed posts are rarely that tall on desktop view.
                            if (rect.height > (window.innerHeight * 0.65)) { 
                                bestVideo = v; 
                                break; 
                            }
                        }
                    }

                    if (bestVideo) {
                        let src = bestVideo.src;
                        if (src.startsWith('blob:')) {
                            // Blob Detected: Find match in network log
                            // We look for the NEWEST large video file.
                            let match = resources.find(r => 
                                (r.url.includes('.mp4') || r.url.includes('.m4a') || r.url.includes('bytestart')) &&
                                r.size > 500000 // >500KB to ensure it's not a preview/highlight
                            );
                            if (match) return "DOM_VIDEO|" + match.url;
                        } else {
                            return "DOM_VIDEO|" + src;
                        }
                    }

                    // --- IMAGE FINDER ---
                    let allImages = Array.from(document.querySelectorAll('img'));
                    for (let img of allImages) {
                        let rect = img.getBoundingClientRect();
                        if (centerX >= rect.left && centerX <= rect.right && centerY >= rect.top && centerY <= rect.bottom) {
                            // Same height check for images
                            if (rect.height > (window.innerHeight * 0.65)) {
                                if (img.srcset) {
                                    let parts = img.srcset.split(',');
                                    let best = parts.pop(); 
                                    return "DOM_IMAGE|" + best.trim().split(' ')[0];
                                }
                                return "DOM_IMAGE|" + img.src;
                            }
                        }
                    }

                    return "EMPTY";
                })()
            "#;

            let raw_result = match self.tab.evaluate(js_identify, false) {
                Ok(res) => res.value.unwrap().as_str().unwrap_or("").to_string(),
                Err(_) => "".to_string(),
            };

            if raw_result == "EMPTY" || raw_result.is_empty() {
                thread::sleep(Duration::from_millis(500));
                continue;
            }

            let parts: Vec<&str> = raw_result.split('|').collect();
            let kind = *parts.get(0).unwrap_or(&"");
            let mut url = parts.get(1).unwrap_or(&"").to_string();
            
            if url.contains("&bytestart") { 
                if let Some(idx) = url.find("&bytestart") { url = url.get(..idx).unwrap_or(&url).to_string(); }
            }

            if history.contains(&url) || failed_urls.contains(&url) { 
                thread::sleep(Duration::from_millis(500));
                continue; 
            }

            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let base_filename = format!("{}_{}", username, timestamp);

            if kind == "DOM_VIDEO" {
                let v_name = format!("{}_v.mp4", base_filename);
                
                if self.fetch_and_save(&url, &v_name).await.is_ok() {
                    
                    // Find Audio: Look for newest audio-like resource that is NOT the video we just found
                    let js_find_audio = format!(r#"
                        (function() {{
                            let resources = performance.getEntriesByType('resource');
                            let audios = resources.filter(r => 
                                (r.name.includes('.mp4') || r.name.includes('.m4a')) && 
                                r.transferSize > 50000 && 
                                r.name !== "{}" 
                            );
                            audios.sort((a,b) => b.startTime - a.startTime);
                            if (audios.length > 0) return audios[0].name;
                            return "";
                        }})()
                    "#, url);
                    
                    let audio_url = match self.tab.evaluate(&js_find_audio, false) {
                        Ok(res) => res.value.unwrap().as_str().unwrap_or("").to_string(),
                        Err(_) => "".to_string(),
                    };

                    let final_name = format!("{}.mp4", base_filename);

                    if !audio_url.is_empty() {
                        let a_name = format!("{}_audio.mp4", base_filename);
                        if self.fetch_and_save(&audio_url, &a_name).await.is_ok() {
                            let _ = mux_video_audio(&v_name, &a_name, &final_name);
                            history.insert(url.clone());
                            history.insert(audio_url);
                            return Ok(true);
                        }
                    }
                    
                    log_info("No separate audio stream found. Saving Video.");
                    let _ = rename_video_only(&v_name, &final_name);
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
                    const response = await fetch("{}", {{ credentials: 'omit' }});
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
