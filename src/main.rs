mod config;
mod browser;
mod instagram;
mod utils;

use std::io::{self, Write};
use utils::{setup_env, log_info, log_error, clear_terminal};
use instagram::InstagramBot;
use browser::launch_browser;
use colored::*; 

#[tokio::main]
async fn main() {
    // 1. BUILD PHASE CHECK
    // The tool has finished compiling. We pause here so you can read any 
    // cargo warnings or errors above before we wipe them.
    println!("Build Complete. Press {} to launch STOV...", "ENTER".yellow().bold());
    let _ = io::stdin().read_line(&mut String::new());

    // 2. NUCLEAR CLEAN START
    clear_terminal();

    setup_env();

    // 3. DISPLAY BANNER
    println!("{}", "======================================".cyan().bold());
    println!("{}", "       STOV - TERMUX EDITION          ".cyan().bold());
    println!("{}", "   State of the Art Observation Tool  ".white().italic());
    println!("{}", "======================================".cyan().bold());
    println!("");

    // 4. Get Inputs
    let mut username = String::new();
    let mut password = String::new();
    let mut targets_input = String::new();

    print!("{} ", "Your Username:".yellow());
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut username).unwrap();

    print!("{} ", "Your Password:".yellow());
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut password).unwrap();

    print!("{} ", "Targets (e.g. user1,user2):".yellow());
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut targets_input).unwrap();

    let targets: Vec<String> = targets_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    println!(""); 
    
    match launch_browser() {
        Ok(browser) => {
            match InstagramBot::new(&browser) {
                Ok(bot) => {
                    if let Err(e) = bot.login(username.trim(), password.trim()) {
                        log_error(&format!("Login Critical Error: {}", e));
                        return;
                    }
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
