use colored::*;
use std::fs;
use std::path::Path;
use std::io::Write;
use anyhow::Result;
use crate::config::DOWNLOAD_DIR;

pub fn setup_env() {
    let path = Path::new(DOWNLOAD_DIR);
    if !path.exists() {
        fs::create_dir_all(path).expect("Failed to create download directory");
    }
}

pub fn log_info(msg: &str) {
    println!("{} {}", "[INFO]".green().bold(), msg);
}

pub fn log_error(msg: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), msg);
}

pub async fn download_file(url: &str, filename: &str) -> Result<()> {
    let path = format!("{}/{}", DOWNLOAD_DIR, filename);
    
    if Path::new(&path).exists() {
        return Ok(()); 
    }

    let response = reqwest::get(url).await?;
    if response.status().is_success() {
        let bytes = response.bytes().await?;
        let mut file = fs::File::create(path)?;
        file.write_all(&bytes)?;
        log_info(&format!("Saved media: {}", filename));
    }
    Ok(())
}
