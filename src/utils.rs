use colored::*;
use std::fs;
use std::path::Path;
use std::io::Write;
use anyhow::Result;
use rand::Rng;
use crate::config::{DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR};

pub fn setup_env() {
    let paths = vec![DOWNLOAD_DIR, IMAGES_DIR, PROOF_DIR, ERROR_DIR];
    for p in paths {
        let path = Path::new(p);
        if !path.exists() {
            if let Err(e) = fs::create_dir_all(path) {
                log_error(&format!("CRITICAL: Failed to create folder {}: {}", p, e));
            }
        }
    }
}

pub fn log_info(msg: &str) {
    println!("{} {}", "[INFO]".green().bold(), msg);
}

pub fn log_error(msg: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), msg);
}

pub fn save_screenshot(data: Vec<u8>, folder: &str, base_name: &str) -> Result<()> {
    // Ensure folder exists right before writing (Paranoia check)
    if !Path::new(folder).exists() {
        fs::create_dir_all(folder)?;
    }

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
    log_error(&format!("Screenshot failed, saved HTML dump instead: {:?}", path));
}

pub async fn download_file(url: &str, filename: &str) -> Result<()> {
    let path = format!("{}/{}", DOWNLOAD_DIR, filename);
    if Path::new(&path).exists() { return Ok(()); }
    let response = reqwest::get(url).await?;
    if response.status().is_success() {
        let bytes = response.bytes().await?;
        let mut file = fs::File::create(path)?;
        file.write_all(&bytes)?;
        log_info(&format!("Media Downloaded: {}", filename));
    }
    Ok(())
}
