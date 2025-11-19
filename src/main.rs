mod config;
mod browser;
mod instagram;
mod utils;

use std::io::{self, Write};
use utils::{setup_env, log_info, log_error};
use instagram::InstagramBot;
use browser::launch_browser;

#[tokio::main]
async fn main() {
    setup_env();

    println!("======================================");
    println!("   INSTA SENTINEL - TERMUX EDITION    ");
    println!("======================================");

    // 1. Get Inputs
    let mut username = String::new();
    let mut password = String::new();
    let mut targets_input = String::new();

    print!("Your Username: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut username).unwrap();

    print!("Your Password: ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut password).unwrap();

    print!("Targets (e.g. user1,user2): ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut targets_input).unwrap();

    let targets: Vec<String> = targets_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // 2. Launch System
    match launch_browser() {
        Ok(browser) => {
            match InstagramBot::new(&browser) {
                Ok(bot) => {
                    // Login
                    if let Err(e) = bot.login(username.trim(), password.trim()) {
                        log_error(&format!("Login Critical Error: {}", e));
                        return;
                    }

                    // Run Scraper
                    if let Err(e) = bot.process_targets(targets).await {
                        log_error(&format!("Scraping Error: {}", e));
                    }
                },
                Err(e) => log_error(&format!("Tab Creation Failed: {}", e)),
            }
        },
        Err(e) => {
            log_error(&format!("Browser Launch Failed: {}", e));
            println!("Ensure you ran: pkg install chromium");
        }
    }
    
    log_info("Operation Completed.");
}
