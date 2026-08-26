use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use colored::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{DOWNLOAD_DIR, ERROR_DIR, IMAGES_DIR, PROFILES_DIR, PROOF_DIR};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserProfile {
    pub username: String,
    pub session_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MediaManifest {
    pub filename: String,
    pub username: String,
    pub story_key: String,
    pub media_type: String,
    pub status: String,
    pub has_audio: bool,
    pub bytes: u64,
    pub created_at: u64,
    pub error: Option<String>,
}

pub fn setup_env() {
    for path in [DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR, PROFILES_DIR] {
        if let Err(error) = fs::create_dir_all(path) {
            log_error(&format!("Could not create {}: {}", path, error));
        }
    }
}

pub fn log_info(message: &str) {
    println!("{} {}", "[INFO]".green().bold(), message);
}

pub fn log_error(message: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), message);
}

pub fn clear_terminal() {
    print!("\x1b[2J\x1b[H");
    let _ = io::stdout().flush();
}

fn set_private_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn unique_temp_path(destination: &Path, suffix: &str) -> PathBuf {
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("stov-artifact");
    destination.with_file_name(format!(".{}.{}.{}", name, rand::random::<u64>(), suffix))
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("Destination has no parent: {}", destination.display()))?;
    fs::create_dir_all(parent)?;

    let temporary = unique_temp_path(destination, "part");
    let result = (|| -> Result<()> {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, destination)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_json_atomic<T: Serialize>(destination: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_atomic(destination, &bytes)
}

pub fn save_profile(username: &str, session_id: &str) -> Result<()> {
    let path = Path::new(PROFILES_DIR).join(format!("{}.json", safe_filename(username)));
    write_json_atomic(
        &path,
        &UserProfile {
            username: username.to_string(),
            session_id: session_id.to_string(),
        },
    )?;
    set_private_permissions(&path)?;
    log_info(&format!("Session saved securely for user: {}", username));
    Ok(())
}

pub fn list_profiles() -> Result<Vec<String>> {
    let mut profiles = Vec::new();
    fs::create_dir_all(PROFILES_DIR)?;
    for entry in fs::read_dir(PROFILES_DIR)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem() {
                profiles.push(stem.to_string_lossy().to_string());
            }
        }
    }
    profiles.sort();
    Ok(profiles)
}

pub fn load_profile_session(username: &str) -> Result<String> {
    let path = Path::new(PROFILES_DIR).join(format!("{}.json", safe_filename(username)));
    let data = fs::read_to_string(&path)
        .with_context(|| format!("Could not read profile {}", path.display()))?;
    let profile: UserProfile = serde_json::from_str(&data)
        .with_context(|| format!("Invalid profile {}", path.display()))?;
    Ok(profile.session_id)
}

pub fn save_screenshot(data: Vec<u8>, folder: &str, base_name: &str) -> Result<()> {
    let path = Path::new(folder).join(format!(
        "{}{}.png",
        safe_filename(base_name),
        rand::thread_rng().gen_range(1000..9999)
    ));
    write_atomic(&path, &data)?;
    log_info(&format!("Evidence saved: {}", path.display()));
    Ok(())
}

pub fn save_html(text: String, folder: &str, base_name: &str) {
    let path = Path::new(folder).join(format!(
        "{}{}.html",
        safe_filename(base_name),
        rand::thread_rng().gen_range(1000..9999)
    ));
    if let Err(error) = write_atomic(&path, text.as_bytes()) {
        log_error(&format!("Could not save HTML evidence: {}", error));
    }
}

pub fn safe_filename(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn is_image_filename(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
}

fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

fn probe_stream_types(path: &Path) -> Result<Vec<String>> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .with_context(|| format!("Could not execute ffprobe for {}", path.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe rejected {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn validate_media_file(path: &Path, require_video: bool, require_audio: bool) -> Result<()> {
    if !path.is_file() {
        return Err(anyhow!("Media file does not exist: {}", path.display()));
    }
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 {
        return Err(anyhow!("Media file is empty: {}", path.display()));
    }

    if is_image_filename(path.to_string_lossy().as_ref()) {
        let header = fs::read(path)?;
        if !looks_like_image(&header) {
            return Err(anyhow!(
                "File is not a recognized image: {}",
                path.display()
            ));
        }
        return Ok(());
    }

    let streams = probe_stream_types(path)?;
    if require_video && !streams.iter().any(|stream| stream == "video") {
        return Err(anyhow!("Media has no video stream: {}", path.display()));
    }
    if require_audio && !streams.iter().any(|stream| stream == "audio") {
        return Err(anyhow!("Media has no audio stream: {}", path.display()));
    }
    if !require_video && !require_audio && streams.is_empty() {
        return Err(anyhow!("Media has no streams: {}", path.display()));
    }
    Ok(())
}

pub fn save_bytes_file(bytes: &[u8], filename: &str) -> Result<()> {
    if bytes.is_empty() {
        return Err(anyhow!("Media payload is empty"));
    }
    let path = Path::new(DOWNLOAD_DIR).join(safe_filename(filename));
    write_atomic(&path, bytes)?;
    if let Err(error) = validate_media_file(&path, false, false) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(())
}

pub fn media_has_audio(path: &Path) -> Result<bool> {
    Ok(probe_stream_types(path)?
        .iter()
        .any(|stream| stream == "audio"))
}

pub fn save_base64_file(base64_string: &str, filename: &str) -> Result<()> {
    let clean_string = base64_string
        .split_once(',')
        .map(|(_, value)| value)
        .unwrap_or(base64_string);
    let bytes = general_purpose::STANDARD
        .decode(clean_string)
        .context("Invalid base64 media payload")?;
    if bytes.is_empty() {
        return Err(anyhow!("Decoded media payload is empty"));
    }

    let path = Path::new(DOWNLOAD_DIR).join(safe_filename(filename));
    write_atomic(&path, &bytes)?;

    if let Err(error) = validate_media_file(&path, false, false) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(())
}

fn run_ffmpeg(args: &[PathOrArg<'_>]) -> Result<()> {
    let mut command = Command::new("ffmpeg");
    command.arg("-hide_banner").arg("-loglevel").arg("error");
    for arg in args {
        match arg {
            PathOrArg::Flag(flag) => {
                command.arg(flag);
            }
            PathOrArg::Path(path) => {
                command.arg(path);
            }
        }
    }
    let output = command
        .stdin(Stdio::null())
        .output()
        .context("Could not execute ffmpeg; install ffmpeg and ffprobe")?;
    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

enum PathOrArg<'a> {
    Flag(&'a str),
    Path(&'a Path),
}

pub fn mux_video_audio(
    video_filename: &str,
    audio_filename: &str,
    final_filename: &str,
) -> Result<()> {
    let video_path = Path::new(DOWNLOAD_DIR).join(safe_filename(video_filename));
    let audio_path = Path::new(DOWNLOAD_DIR).join(safe_filename(audio_filename));
    let final_path = Path::new(DOWNLOAD_DIR).join(safe_filename(final_filename));
    let temporary_output = unique_temp_path(&final_path, "mux.mp4");

    validate_media_file(&video_path, true, false)?;
    validate_media_file(&audio_path, false, true)?;

    let args = [
        PathOrArg::Flag("-y"),
        PathOrArg::Flag("-i"),
        PathOrArg::Path(&video_path),
        PathOrArg::Flag("-i"),
        PathOrArg::Path(&audio_path),
        PathOrArg::Flag("-map"),
        PathOrArg::Flag("0:v:0"),
        PathOrArg::Flag("-map"),
        PathOrArg::Flag("1:a:0"),
        PathOrArg::Flag("-c:v"),
        PathOrArg::Flag("copy"),
        PathOrArg::Flag("-c:a"),
        PathOrArg::Flag("aac"),
        PathOrArg::Flag("-b:a"),
        PathOrArg::Flag("160k"),
        PathOrArg::Flag("-movflags"),
        PathOrArg::Flag("+faststart"),
        PathOrArg::Path(&temporary_output),
    ];

    let result = (|| -> Result<()> {
        run_ffmpeg(&args)?;
        validate_media_file(&temporary_output, true, true)?;
        fs::rename(&temporary_output, &final_path)?;
        Ok(())
    })();

    if result.is_ok() {
        let _ = fs::remove_file(&video_path);
        let _ = fs::remove_file(&audio_path);
    } else {
        let _ = fs::remove_file(&temporary_output);
    }
    result
}

pub fn publish_video_only(temp_filename: &str, final_filename: &str) -> Result<()> {
    let source = Path::new(DOWNLOAD_DIR).join(safe_filename(temp_filename));
    let destination = Path::new(DOWNLOAD_DIR).join(safe_filename(final_filename));
    validate_media_file(&source, true, false)?;
    fs::rename(&source, &destination)?;
    Ok(())
}

pub fn write_manifest(manifest: &MediaManifest) -> Result<()> {
    let path = Path::new(DOWNLOAD_DIR).join(format!("{}.json", safe_filename(&manifest.filename)));
    write_json_atomic(&path, manifest)
}

pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub fn ensure_media_tools() -> Result<()> {
    for command_name in ["ffmpeg", "ffprobe"] {
        let output = Command::new(command_name)
            .arg("-version")
            .output()
            .with_context(|| format!("{} is not installed or not on PATH", command_name))?;
        if !output.status.success() {
            return Err(anyhow!("{} failed its startup check", command_name));
        }
        if let Some(version) = String::from_utf8_lossy(&output.stdout).lines().next() {
            log_info(&format!("{} detected: {}", command_name, version));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_filename_removes_path_separators_and_specials() {
        assert_eq!(safe_filename("../../hello world"), ".._.._hello_world");
        assert_eq!(safe_filename(""), "unknown");
    }

    #[test]
    fn image_signatures_are_recognized() {
        assert!(looks_like_image(&[0xff, 0xd8, 0xff, 0x00]));
        assert!(looks_like_image(b"\x89PNG\r\n\x1a\nrest"));
        assert!(looks_like_image(b"RIFF1234WEBPrest"));
        assert!(!looks_like_image(b"not-an-image"));
    }
}
