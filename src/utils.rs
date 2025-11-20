use colored::*;
use std::fs;
use std::path::Path;
use std::io::Write;
use anyhow::{Result, anyhow};
use rand::Rng;
use base64::{Engine as _, engine::general_purpose}; 
// Config imports
use crate::config::{DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR};

pub fn setup_env() {
    let paths = vec![DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR];
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

// --- EXPERT: SAVE BASE64 VIDEO ---
pub fn save_base64_file(base64_string: &str, filename: &str) -> Result<()> {
    let path = format!("{}/{}", DOWNLOAD_DIR, filename);
    
    // 1. Strip prefix
    let clean_string = if let Some(index) = base64_string.find(',') {
        &base64_string[index + 1..]
    } else {
        base64_string
    };

    // 2. Decode
    let bytes = general_purpose::STANDARD.decode(clean_string)?;

    // 3. VALIDATION (The Fix)
    // A real Instagram Story video is roughly 1MB - 5MB.
    // 209KB is just a header. We reject anything under 500KB for MP4s.
    let min_size = if filename.ends_with(".mp4") { 500_000 } else { 20_000 };

    if bytes.len() < min_size {
        return Err(anyhow!("Decoded file too small ({} KB). Rejected.", bytes.len() / 1024));
    }

    let mut file = fs::File::create(&path)?;
    file.write_all(&bytes)?;
    
    log_info(&format!("Media Saved & Verified (Size: {} KB): {}", bytes.len() / 1024, filename));
    Ok(())
}
