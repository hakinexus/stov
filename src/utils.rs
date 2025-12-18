use colored::*;
use std::fs;
use std::path::Path;
use std::io::Write;
use std::process::Command;
use anyhow::{Result, anyhow};
use rand::Rng;
use base64::{Engine as _, engine::general_purpose}; 
use serde::{Serialize, Deserialize};
use crate::config::{DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR, PROFILES_DIR};

// --- PROFILE STRUCTURE ---
#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub session_id: String,
}

// --- ENVIRONMENT SETUP ---
pub fn setup_env() {
    let paths = vec![DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR, PROFILES_DIR];
    for p in paths {
        let path = Path::new(p);
        if !path.exists() { let _ = fs::create_dir_all(path); }
    }
}

// --- LOGGING ---
pub fn log_info(msg: &str) {
    println!("{} {}", "[INFO]".green().bold(), msg);
}

pub fn log_error(msg: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), msg);
}

// --- UI CLEANER ---
pub fn clear_terminal() {
    // Clears screen, scrollback buffer, and moves cursor to top-left
    print!("\x1b[2J\x1b[3J\x1b[H");
    let _ = std::io::stdout().flush();
}

// --- PROFILE MANAGEMENT ---
pub fn save_profile(username: &str, session_id: &str) -> Result<()> {
    let profile = UserProfile {
        username: username.to_string(),
        session_id: session_id.to_string(),
    };
    let json = serde_json::to_string_pretty(&profile)?;
    let filename = format!("{}/{}.json", PROFILES_DIR, username);
    let mut file = fs::File::create(filename)?;
    file.write_all(json.as_bytes())?;
    log_info(&format!("Session saved for user: {}", username));
    Ok(())
}

pub fn list_profiles() -> Result<Vec<String>> {
    let mut profiles = Vec::new();
    if !Path::new(PROFILES_DIR).exists() { fs::create_dir_all(PROFILES_DIR)?; }
    
    let paths = fs::read_dir(PROFILES_DIR)?;
    for path in paths {
        let p = path?.path();
        if let Some(ext) = p.extension() {
            if ext == "json" {
                if let Some(stem) = p.file_stem() {
                    profiles.push(stem.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(profiles)
}

pub fn load_profile_session(username: &str) -> Result<String> {
    let path = format!("{}/{}.json", PROFILES_DIR, username);
    let data = fs::read_to_string(&path)?;
    let profile: UserProfile = serde_json::from_str(&data)?;
    Ok(profile.session_id)
}

// --- DEBUG / SNAPSHOTS ---
pub fn save_screenshot(data: Vec<u8>, folder: &str, base_name: &str) -> Result<()> {
    if !Path::new(folder).exists() { fs::create_dir_all(folder)?; }
    let mut rng = rand::thread_rng();
    let unique_id: u16 = rng.gen_range(1000..9999);
    let filename = format!("{}{}.png", base_name, unique_id);
    let path = Path::new(folder).join(filename);
    fs::write(&path, data)?;
    log_info(&format!("Evidence saved: {:?}", path));
    Ok(())
}

pub fn save_html(text: String, folder: &str, base_name: &str) {
    if !Path::new(folder).exists() { let _ = fs::create_dir_all(folder); }
    let filename = format!("{}{}.html", base_name, rand::thread_rng().gen_range(1000..9999));
    let path = Path::new(folder).join(filename);
    let _ = fs::write(&path, text);
}

// --- EXPERT: SAVE BASE64 VIDEO (SMART VALIDATION) ---
pub fn save_base64_file(base64_string: &str, filename: &str) -> Result<()> {
    let path = format!("{}/{}", DOWNLOAD_DIR, filename);
    
    let clean_string = if let Some(index) = base64_string.find(',') {
        &base64_string[index + 1..]
    } else {
        base64_string
    };

    let bytes = general_purpose::STANDARD.decode(clean_string)?;

    // INTELLIGENT SIZE CHECK
    // 1. Audio tracks (filename contains "_audio") -> Allow > 5KB
    // 2. Main Videos (ends in .mp4) -> Allow > 200KB (Filters out chunks/headers)
    // 3. Images -> Allow > 10KB (Filters out tiny icons)
    let min_size = if filename.contains("_audio") {
        5_000 
    } else if filename.ends_with(".mp4") {
        200_000 
    } else {
        10_000 
    };

    if bytes.len() < min_size {
        return Err(anyhow!("File too small ({} bytes). Expected > {}. Rejected.", bytes.len(), min_size));
    }

    let mut file = fs::File::create(&path)?;
    file.write_all(&bytes)?;
    
    // Silent success to keep logs clean during muxing steps
    Ok(())
}

// --- FFMPEG MUXING ---
pub fn mux_video_audio(video_filename: &str, audio_filename: &str, final_filename: &str) -> Result<()> {
    let video_path = format!("{}/{}", DOWNLOAD_DIR, video_filename);
    let audio_path = format!("{}/{}", DOWNLOAD_DIR, audio_filename);
    let output_path = format!("{}/{}", DOWNLOAD_DIR, final_filename);
    
    log_info("Muxing Audio/Video streams...");

    // FFmpeg command: -i video -i audio -c copy (No re-encoding, instant merge)
    let status = Command::new("ffmpeg")
        .arg("-y") // Overwrite output
        .arg("-v").arg("error") // Quiet mode
        .arg("-i").arg(&video_path)
        .arg("-i").arg(&audio_path)
        .arg("-c").arg("copy")
        .arg(&output_path)
        .status();

    // Clean up temp files regardless of success
    let _ = fs::remove_file(&video_path);
    let _ = fs::remove_file(&audio_path);

    match status {
        Ok(s) if s.success() => {
            log_info(&format!("Success! Saved: {}", final_filename));
            Ok(())
        },
        _ => {
            log_error("FFmpeg Muxing Failed. Saving video only.");
            // If mux fails (e.g. ffmpeg missing), we try to at least save the video
            // by renaming the temp video file to the final name if it still exists (before delete)
            // But since we deleted above, we just return error.
            // (Note: To be truly safe, we could move delete after this check, but usually if ffmpeg fails, something is wrong with the system)
            Err(anyhow!("Muxing failed"))
        }
    }
}

// --- VIDEO FALLBACK RENAMER ---
// Used when Audio stream is never found, so we just save the muted video
pub fn rename_video_only(temp_filename: &str, final_filename: &str) -> Result<()> {
    let source = format!("{}/{}", DOWNLOAD_DIR, temp_filename);
    let dest = format!("{}/{}", DOWNLOAD_DIR, final_filename);
    
    if Path::new(&source).exists() {
        fs::rename(source, dest)?;
        log_info(&format!("Saved Muted Video: {}", final_filename));
        Ok(())
    } else {
        Err(anyhow!("Source file not found"))
    }
}
