use colored::*;
use std::fs;
use std::path::Path;
use std::io::Write;
use anyhow::{Result, anyhow};
use rand::Rng;
use base64::{Engine as _, engine::general_purpose}; 
use serde::{Serialize, Deserialize};
use crate::config::{DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR, PROFILES_DIR};


#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub session_id: String,
}

pub fn setup_env() {
    let paths = vec![DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR, PROFILES_DIR];
    for p in paths {
        let path = Path::new(p);
        if !path.exists() { let _ = fs::create_dir_all(path); }
    }
}

pub fn log_info(msg: &str) {
    println!("{} {}", "[INFO]".green().bold(), msg);
}

pub fn log_error(msg: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), msg);
}


pub fn clear_terminal() {
    print!("\x1b[2J\x1b[3J\x1b[H");
    let _ = std::io::stdout().flush();
}

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

    if !Path::new(PROFILES_DIR).exists() {
        fs::create_dir_all(PROFILES_DIR)?;
    }
    
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

pub fn save_base64_file(base64_string: &str, filename: &str) -> Result<()> {
    let path = format!("{}/{}", DOWNLOAD_DIR, filename);
    
    let clean_string = if let Some(index) = base64_string.find(',') {
        &base64_string[index + 1..]
    } else {
        base64_string
    };

    let bytes = general_purpose::STANDARD.decode(clean_string)?;

    
    let min_size = if filename.ends_with(".mp4") { 200_000 } else { 15_000 };

    if bytes.len() < min_size {
        return Err(anyhow!("File too small ({} bytes). Rejected.", bytes.len()));
    }

    let mut file = fs::File::create(&path)?;
    file.write_all(&bytes)?;
    
    log_info(&format!("Media Saved via Browser Fetch (Size: {} KB): {}", bytes.len() / 1024, filename));
    Ok(())
}
