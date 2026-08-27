use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use headless_chrome::protocol::cdp::Network::events::ResponseReceivedEventParams;
use headless_chrome::{Browser, Element, Tab};
use rand::Rng;
use serde::Deserialize;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use url::Url;

use crate::config::*;
use crate::utils::{
    ensure_media_tools, log_error, log_info, media_has_audio, mux_video_audio, now_millis,
    publish_video_only, safe_filename, save_bytes_file, save_profile, save_screenshot,
    validate_media_file, write_manifest, MediaManifest,
};

const MAX_CAPTURE_BYTES: usize = 80 * 1024 * 1024;
const MAX_CAPTURED_ITEMS: usize = 32;

#[derive(Clone)]
struct CapturedMedia {
    url: String,
    mime_type: String,
    body: Vec<u8>,
}

#[derive(Clone, Default)]
struct MediaCapture {
    items: Arc<Mutex<Vec<CapturedMedia>>>,
}

impl MediaCapture {
    fn clear(&self) {
        if let Ok(mut items) = self.items.lock() {
            items.clear();
        }
    }

    fn push(&self, item: CapturedMedia) {
        if item.body.is_empty() || item.body.len() > MAX_CAPTURE_BYTES {
            return;
        }
        if let Ok(mut items) = self.items.lock() {
            items.push(item);
            if items.len() > MAX_CAPTURED_ITEMS {
                let remove_count = items.len() - MAX_CAPTURED_ITEMS;
                items.drain(0..remove_count);
            }
        }
    }

    fn matching_items(&self, wanted: &str) -> Vec<CapturedMedia> {
        self.items
            .lock()
            .ok()
            .map(|items| {
                items
                    .iter()
                    .rev()
                    .filter(|item| {
                        let mime = item.mime_type.to_ascii_lowercase();
                        let url = item.url.to_ascii_lowercase();
                        if wanted == "video" {
                            mime.starts_with("video/")
                                || (mime == "application/octet-stream"
                                    && (url.contains(".mp4") || url.contains("bytestart")))
                        } else {
                            mime.starts_with("audio/")
                                || url.contains(".m4a")
                                || url.contains("mime=audio")
                                || url.contains("audio%2f")
                        }
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
struct ViewerState {
    active: bool,
    kind: String,
    source: String,
    poster: String,
    href: String,
}

impl ViewerState {
    fn key(&self) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.kind.hash(&mut hasher);
        self.source.hash(&mut hasher);
        self.poster.hash(&mut hasher);
        self.href.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

pub struct InstagramBot<'a> {
    _browser: &'a Browser,
    tab: Arc<Tab>,
    capture: MediaCapture,
}

impl<'a> InstagramBot<'a> {
    pub fn new(browser: &'a Browser) -> Result<Self> {
        let tab = browser.new_tab()?;
        let capture = MediaCapture::default();
        let callback_capture = capture.clone();

        tab.register_response_handling(
            "stov-media-capture",
            Box::new(move |response: ResponseReceivedEventParams, fetch_body| {
                let mime_type = response.response.mime_type.to_ascii_lowercase();
                let url = response.response.url.clone();
                let url_lower = url.to_ascii_lowercase();
                let media_like = mime_type.starts_with("video/")
                    || mime_type.starts_with("audio/")
                    || mime_type.starts_with("image/")
                    || url_lower.contains(".mp4")
                    || url_lower.contains(".m4a")
                    || url_lower.contains("mime=video")
                    || url_lower.contains("mime=audio");
                if !media_like {
                    return;
                }

                // The crate documents that the body may not be ready at responseReceived time.
                thread::sleep(Duration::from_millis(800));
                if let Ok(body) = fetch_body() {
                    let bytes = if body.base_64_encoded {
                        general_purpose::STANDARD
                            .decode(body.body)
                            .unwrap_or_default()
                    } else {
                        body.body.into_bytes()
                    };
                    callback_capture.push(CapturedMedia {
                        url,
                        mime_type,
                        body: bytes,
                    });
                }
            }),
        )?;

        Ok(Self {
            _browser: browser,
            tab,
            capture,
        })
    }

    fn find_now(
        &self,
        css_selectors: &[&str],
        xpath_selectors: &[&str],
    ) -> Option<(Element<'_>, String)> {
        for selector in css_selectors {
            if let Ok(mut elements) = self.tab.find_elements(selector) {
                if let Some(element) = elements.pop() {
                    return Some((element, (*selector).to_string()));
                }
            }
        }
        for selector in xpath_selectors {
            if let Ok(mut elements) = self.tab.find_elements_by_xpath(selector) {
                if let Some(element) = elements.pop() {
                    return Some((element, (*selector).to_string()));
                }
            }
        }
        None
    }

    fn wait_for_field(
        &self,
        label: &str,
        css_selectors: &[&str],
        xpath_selectors: &[&str],
        timeout: Duration,
    ) -> Result<Element<'_>> {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            if let Some((element, selector)) = self.find_now(css_selectors, xpath_selectors) {
                log_info(&format!("{} field matched selector: {}", label, selector));
                return Ok(element);
            }
            thread::sleep(Duration::from_millis(250));
        }
        self.snapshot(
            ERROR_DIR,
            &format!("login_{}_timeout", safe_filename(label)),
        );
        Err(anyhow!(
            "Timed out waiting for the {} field after {} seconds",
            label,
            timeout.as_secs()
        ))
    }

    fn snapshot(&self, folder: &str, name: &str) {
        match self.tab.capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        ) {
            Ok(png) => {
                let _ = save_screenshot(png, folder, name);
            }
            Err(_) => {
                if let Ok(content) = self.tab.get_content() {
                    crate::utils::save_html(content, folder, name);
                }
            }
        }
    }

    fn react_type(&self, element: &Element, text: &str) -> Result<()> {
        element.click()?;
        self.tab.type_str(text)?;
        thread::sleep(Duration::from_millis(150));
        Ok(())
    }

    fn safely_click_login(&self) -> Result<()> {
        log_info("Activating login...");
        if let Ok(buttons) = self.tab.find_elements("button") {
            for button in buttons {
                if let Ok(text) = button.get_inner_text() {
                    let text = text.to_ascii_lowercase();
                    if text.contains("log in") && !text.contains("show") {
                        button.click()?;
                        return Ok(());
                    }
                }
            }
        }
        if let Ok(button) = self.tab.find_element(SEL_SUBMIT) {
            if !button
                .get_inner_text()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains("show")
            {
                button.click()?;
                return Ok(());
            }
        }
        self.tab.press_key("Enter")?;
        Ok(())
    }

    pub fn login_with_session(&self, session_id: &str) -> Result<()> {
        log_info("Attempting login via saved session...");
        self.tab.navigate_to("https://www.instagram.com")?;
        let cookie = headless_chrome::protocol::cdp::Network::CookieParam {
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
        self.tab.reload(true, None)?;
        thread::sleep(Duration::from_secs(4));

        if self.tab.find_element(SEL_HOME_ICON).is_ok()
            || self.tab.find_element(SEL_AVATAR).is_ok()
            || !self.tab.get_url().contains("accounts/login")
        {
            log_info("Session login successful.");
            return Ok(());
        }
        Err(anyhow!("Session expired or invalid; please log in again."))
    }

    pub fn login(&self, user: &str, pass: &str) -> Result<()> {
        self.tab
            .navigate_to("https://www.instagram.com/accounts/login/")?;
        thread::sleep(Duration::from_secs(5));

        for xpath in [
            "//button[contains(text(), 'Allow all cookies')]",
            "//button[contains(text(), 'Allow')]",
            "//button[contains(text(), 'Decline')]",
        ] {
            if let Ok(element) = self.tab.find_element_by_xpath(xpath) {
                let _ = element.click();
                thread::sleep(Duration::from_millis(500));
                break;
            }
        }

        log_info("Waiting for username field...");
        let user_element = self.wait_for_field(
            "username",
            USER_CSS_SELECTORS,
            USER_XPATH_SELECTORS,
            Duration::from_secs(45),
        )?;
        self.react_type(&user_element, user)?;
        log_info("Username entered. Waiting for password field...");
        let password_element = self.wait_for_field(
            "password",
            PASS_CSS_SELECTORS,
            PASS_XPATH_SELECTORS,
            Duration::from_secs(45),
        )?;
        self.react_type(&password_element, pass)?;
        thread::sleep(Duration::from_secs(1));
        self.safely_click_login()?;

        for _ in 0..40 {
            let url = self.tab.get_url();
            if (self.tab.find_element(SEL_HOME_ICON).is_ok()
                || self.tab.find_element(SEL_AVATAR).is_ok())
                && !url.contains("accounts/login")
            {
                if let Ok(cookies) = self.tab.get_cookies() {
                    if let Some(cookie) = cookies
                        .into_iter()
                        .find(|cookie| cookie.name == "sessionid")
                    {
                        save_profile(user, &cookie.value)?;
                    }
                }
                log_info("Login verified.");
                return Ok(());
            }
            if let Ok(alert) = self.tab.find_element("p[role='alert']") {
                if let Ok(text) = alert.get_inner_text() {
                    let lower = text.to_ascii_lowercase();
                    if lower.contains("incorrect") {
                        return Err(anyhow!("Instagram rejected the credentials"));
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }

        self.snapshot(ERROR_DIR, "login_timeout");
        Err(anyhow!("Login timed out"))
    }

    fn find_story_entry(&self) -> Option<Element<'_>> {
        for selector in STORY_ENTRY_SELECTORS {
            if let Ok(element) = self.tab.find_element(selector) {
                log_info(&format!("Story entry matched selector: {}", selector));
                return Some(element);
            }
        }
        None
    }

    fn viewer_state(&self) -> Result<ViewerState> {
        let script = r#"
            (function() {
                function visible(el) {
                    const r = el.getBoundingClientRect();
                    const s = getComputedStyle(el);
                    return r.width > 20 && r.height > 20 && s.display !== 'none' && s.visibility !== 'hidden';
                }
                const media = Array.from(document.querySelectorAll('video, img'))
                    .filter(visible)
                    .map(el => {
                        const r = el.getBoundingClientRect();
                        const kind = el.tagName.toLowerCase();
                        const source = kind === 'video'
                            ? (el.currentSrc || el.src || (el.querySelector('source') || {}).src || '')
                            : (el.currentSrc || el.src || '');
                        return {
                            kind,
                            source,
                            poster: el.poster || '',
                            area: r.width * r.height,
                            width: r.width,
                            height: r.height
                        };
                    })
                    .filter(item => item.width >= 200 && item.height >= 200)
                    .sort((a, b) => b.area - a.area);
                const target = media[0];
                        if (!target) return { active: false, kind: '', source: '', poster: '', href: location.pathname };
                return {
                    active: location.pathname.includes('/stories/'),
                    kind: target.kind,
                    source: target.source,
                    poster: target.poster,
                    href: location.pathname
                };
            })()
        "#;
        let result = self.tab.evaluate(script, false)?;
        let value = result
            .value
            .ok_or_else(|| anyhow!("Viewer state returned no value"))?;
        serde_json::from_value(value).context("Invalid viewer state returned by browser")
    }

    fn wait_for_viewer(&self, timeout: Duration) -> Result<Option<ViewerState>> {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            if let Ok(state) = self.viewer_state() {
                if state.active {
                    return Ok(Some(state));
                }
            }
            thread::sleep(Duration::from_millis(300));
        }
        Ok(None)
    }

    fn click_next(&self) -> Result<bool> {
        for selector in STORY_NEXT_SELECTORS {
            if let Ok(element) = self.tab.find_element(selector) {
                if element.click().is_ok() {
                    return Ok(true);
                }
            }
        }
        Ok(self.tab.press_key("ArrowRight").is_ok())
    }

    fn wait_for_transition(&self, previous_key: &str, timeout: Duration) -> Result<bool> {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            if let Ok(state) = self.viewer_state() {
                if !state.active || state.key() != previous_key {
                    return Ok(true);
                }
            }
            thread::sleep(Duration::from_millis(250));
        }
        Ok(false)
    }

    pub async fn process_targets(&self, targets: Vec<String>) -> Result<()> {
        ensure_media_tools()?;
        for target in targets {
            let target = target.trim().trim_matches('/').trim_start_matches('@');
            if target.is_empty() {
                continue;
            }
            log_info(&format!("Checking target: {}", target));
            let profile_url = format!("https://www.instagram.com/{}/", target);
            if self.tab.navigate_to(&profile_url).is_err() {
                log_error(&format!("Could not navigate to {}", target));
                continue;
            }
            thread::sleep(Duration::from_secs(3));

            self.capture.clear();
            if let Some(entry) = self.find_story_entry() {
                let _ = entry.click();
                thread::sleep(Duration::from_secs(2));
            }

            let viewer_url = format!("https://www.instagram.com/stories/{}/", target);
            if self.wait_for_viewer(Duration::from_secs(5))?.is_none() {
                log_info("Click did not open a viewer; trying the direct story route.");
                self.capture.clear();
                let _ = self.tab.navigate_to(&viewer_url);
                thread::sleep(Duration::from_secs(3));
            }

            if let Err(error) = self.process_story_batch(target).await {
                log_error(&format!("Target {} failed: {}", target, error));
            }
            thread::sleep(Duration::from_secs(rand::thread_rng().gen_range(2..5)));
        }
        Ok(())
    }

    async fn process_story_batch(&self, username: &str) -> Result<()> {
        let mut story_count = 0usize;
        let mut consecutive_errors = 0usize;
        let mut state = match self.wait_for_viewer(Duration::from_secs(12))? {
            Some(state) => state,
            None => {
                self.snapshot(
                    ERROR_DIR,
                    &format!("{}_viewer_timeout", safe_filename(username)),
                );
                return Ok(());
            }
        };

        for _ in 0..100 {
            if !state.active {
                break;
            }
            let key = state.key();
            match self.download_active_story(username, &key, &state).await {
                Ok(true) => {
                    story_count += 1;
                    consecutive_errors = 0;
                    log_info(&format!("Story #{} saved.", story_count));
                }
                Ok(false) => {
                    consecutive_errors += 1;
                    log_error("Story media did not become downloadable or failed validation.");
                }
                Err(error) => {
                    consecutive_errors += 1;
                    log_error(&format!("Story failed: {}", error));
                }
            }

            if consecutive_errors >= 7 {
                self.snapshot(
                    ERROR_DIR,
                    &format!("{}_batch_stopped", safe_filename(username)),
                );
                break;
            }

            self.capture.clear();
            let previous_key = key.clone();
            if !self.click_next()? {
                break;
            }
            if !self.wait_for_transition(&previous_key, Duration::from_secs(8))? {
                log_info("No observable next-story transition; retrying once.");
                let _ = self.click_next()?;
                if !self.wait_for_transition(&previous_key, Duration::from_secs(5))? {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(800));
            state = match self.wait_for_viewer(Duration::from_secs(5))? {
                Some(next) => next,
                None => break,
            };
        }

        let _ = self.tab.press_key("Escape");
        log_info(&format!("Batch complete. Total saved: {}", story_count));
        Ok(())
    }

    async fn download_active_story(
        &self,
        username: &str,
        story_key: &str,
        state: &ViewerState,
    ) -> Result<bool> {
        let stem = format!("{}_{}_{}", safe_filename(username), now_millis(), story_key);
        let mut attempts = 0usize;
        while attempts < 60 {
            attempts += 1;
            let current = self.viewer_state().unwrap_or_else(|_| state.clone());
            if !current.active {
                return Ok(false);
            }

            if current.kind == "img" {
                if let Some(url) = http_url(&current.source) {
                    let filename = format!("{}.jpg", stem);
                    if self.fetch_and_save(&url, &filename).await.is_ok() {
                        let path = std::path::Path::new(DOWNLOAD_DIR).join(&filename);
                        validate_media_file(&path, false, false)?;
                        self.publish_manifest(username, story_key, &filename, "image", true, None)?;
                        return Ok(true);
                    }
                }
            } else if current.kind == "video" {
                let video_filename = format!("{}.video.mp4", stem);
                let video_path = std::path::Path::new(DOWNLOAD_DIR).join(&video_filename);
                let mut saved_video = false;

                if let Some(url) = http_url(&current.source) {
                    if self.fetch_and_save(&url, &video_filename).await.is_ok()
                        && validate_media_file(&video_path, true, false).is_ok()
                    {
                        saved_video = true;
                    }
                }

                if !saved_video {
                    for captured in self.capture.matching_items("video") {
                        if save_bytes_file(&captured.body, &video_filename).is_ok()
                            && validate_media_file(&video_path, true, false).is_ok()
                        {
                            saved_video = true;
                            break;
                        }
                    }
                }

                if saved_video {
                    let final_filename = format!("{}.mp4", stem);
                    if media_has_audio(&video_path).unwrap_or(false) {
                        publish_video_only(&video_filename, &final_filename)?;
                        self.publish_manifest(
                            username,
                            story_key,
                            &final_filename,
                            "video",
                            true,
                            None,
                        )?;
                        return Ok(true);
                    }

                    for _ in 0..12 {
                        for audio in self.capture.matching_items("audio") {
                            let audio_filename = format!("{}.audio.mp4", stem);
                            if save_bytes_file(&audio.body, &audio_filename).is_ok() {
                                let audio_path =
                                    std::path::Path::new(DOWNLOAD_DIR).join(&audio_filename);
                                if validate_media_file(&audio_path, false, true).is_ok()
                                    && mux_video_audio(
                                        &video_filename,
                                        &audio_filename,
                                        &final_filename,
                                    )
                                    .is_ok()
                                {
                                    self.publish_manifest(
                                        username,
                                        story_key,
                                        &final_filename,
                                        "video",
                                        true,
                                        None,
                                    )?;
                                    return Ok(true);
                                }
                            }
                        }
                        thread::sleep(Duration::from_millis(500));
                    }

                    // A valid video-only artifact is preferable to a false “complete” mux result.
                    publish_video_only(&video_filename, &final_filename)?;
                    self.publish_manifest(
                        username,
                        story_key,
                        &final_filename,
                        "video",
                        false,
                        Some("No validated audio track was captured"),
                    )?;
                    return Ok(true);
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
        Ok(false)
    }

    fn publish_manifest(
        &self,
        username: &str,
        story_key: &str,
        filename: &str,
        media_type: &str,
        has_audio: bool,
        error: Option<&str>,
    ) -> Result<()> {
        let path = std::path::Path::new(DOWNLOAD_DIR).join(filename);
        let bytes = std::fs::metadata(&path)?.len();
        write_manifest(&MediaManifest {
            filename: filename.to_string(),
            username: username.to_string(),
            story_key: story_key.to_string(),
            media_type: media_type.to_string(),
            status: if error.is_some() {
                "video-only"
            } else {
                "complete"
            }
            .to_string(),
            has_audio,
            bytes,
            created_at: crate::utils::now_unix(),
            error: error.map(ToOwned::to_owned),
        })
    }

    async fn fetch_and_save(&self, url: &str, filename: &str) -> Result<()> {
        let url_literal = serde_json::to_string(url)?;
        let script = format!(
            r#"(async function() {{
                try {{
                    const response = await fetch({}, {{
                        credentials: 'include',
                        cache: 'no-store',
                        redirect: 'follow'
                    }});
                    if (!response.ok) return "ERROR_HTTP_" + response.status;
                    const blob = await response.blob();
                    return await new Promise((resolve, reject) => {{
                        const reader = new FileReader();
                        reader.onerror = () => reject(new Error('FileReader failed'));
                        reader.onloadend = () => resolve(reader.result);
                        reader.readAsDataURL(blob);
                    }});
                }} catch (error) {{ return "ERROR_CATCH:" + error.message; }}
            }})()"#,
            url_literal
        );

        let result = self.tab.evaluate(&script, true)?;
        let value = result
            .value
            .ok_or_else(|| anyhow!("Browser fetch returned no value"))?;
        let data = value.as_str().unwrap_or_default();
        if !data.starts_with("data:") {
            return Err(anyhow!("Browser fetch failed: {}", data));
        }
        crate::utils::save_base64_file(data, filename)
    }
}

fn http_url(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return None;
    }
    Some(canonical_media_url(parsed))
}

fn canonical_media_url(mut url: Url) -> String {
    let retained: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| key != "bytestart" && key != "bytestop")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    if !retained.is_empty() {
        url.query_pairs_mut().extend_pairs(retained);
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_key_changes_when_media_changes_without_url_change() {
        let first = ViewerState {
            active: true,
            kind: "video".to_string(),
            source: "blob:first".to_string(),
            poster: String::new(),
            href: "https://www.instagram.com/stories/example/".to_string(),
        };
        let second = ViewerState {
            source: "blob:second".to_string(),
            ..first.clone()
        };
        assert_ne!(first.key(), second.key());
    }

    #[test]
    fn blob_sources_are_left_for_cdp_capture() {
        assert!(http_url("blob:https://instagram.com/abc").is_none());
        assert_eq!(
            http_url("https://cdn.example/media.mp4").as_deref(),
            Some("https://cdn.example/media.mp4")
        );
    }

    #[test]
    fn canonicalization_preserves_auth_parameters() {
        let url = http_url("https://cdn.example/media.mp4?bytestart=0&bytestop=99&oe=abc&st=xyz")
            .expect("valid media URL");
        assert_eq!(url, "https://cdn.example/media.mp4?oe=abc&st=xyz");
    }
}

#[cfg(test)]
mod login_selector_tests {
    use super::*;

    #[test]
    fn password_selectors_cover_current_and_legacy_attributes() {
        assert!(PASS_CSS_SELECTORS.contains(&"input[type='password']"));
        assert!(PASS_CSS_SELECTORS.contains(&"input[autocomplete='current-password']"));
        assert!(PASS_CSS_SELECTORS.contains(&"input[aria-label*='password' i]"));
        assert!(PASS_XPATH_SELECTORS
            .iter()
            .any(|selector| selector.contains("@type='password'")));
        assert!(PASS_XPATH_SELECTORS
            .iter()
            .any(|selector| selector.contains("@placeholder")));
    }
}
