mod config;
mod browser;
mod instagram;
mod utils;

use std::io::{self, Write};
use utils::{setup_env, log_info, log_error, clear_terminal, list_profiles, load_profile_session};
use instagram::InstagramBot;
use browser::launch_browser;
use colored::*; 

#[tokio::main]
async fn main() {
    println!("Build Complete. Press {} to launch STOV...", "ENTER".yellow().bold());
    let _ = io::stdin().read_line(&mut String::new());

    clear_terminal();
    setup_env();

    println!("{}", "======================================".cyan().bold());
    println!("{}", "       STOV - TERMUX EDITION          ".cyan().bold());
    println!("{}", "   State of the Art Observation Tool  ".white().italic());
    println!("{}", "======================================".cyan().bold());
    println!("");

    // --- CREDENTIAL LOGIC ---
    let mut username = String::new();
    let mut password = String::new();
    let mut use_saved_session = false;
    let mut saved_session_id = String::new();

    // 1. Check for Saved Profiles
    let profiles = list_profiles().unwrap_or_default();
    
    if !profiles.is_empty() {
        println!("Saved Profiles Found:");
        println!("1. Login with New Account");
        println!("2. Use Saved Account");
        print!("\nSelect Option (1/2): ");
        io::stdout().flush().unwrap();
        
        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        
        if choice.trim() == "2" {
            println!("\nSelect Profile:");
            for (i, prof) in profiles.iter().enumerate() {
                println!("{}. {}", i + 1, prof);
            }
            print!("Enter Number: ");
            io::stdout().flush().unwrap();
            
            let mut prof_choice = String::new();
            io::stdin().read_line(&mut prof_choice).unwrap();
            
            if let Ok(idx) = prof_choice.trim().parse::<usize>() {
                if idx > 0 && idx <= profiles.len() {
                    let selected_user = &profiles[idx - 1];
                    if let Ok(sid) = load_profile_session(selected_user) {
                        log_info(&format!("Loaded session for {}", selected_user));
                        saved_session_id = sid;
                        use_saved_session = true;
                    } else {
                        log_error("Failed to load session. Switching to manual login.");
                    }
                }
            }
        }
    }

    // 2. Manual Inputs (if not using session)
    if !use_saved_session {
        print!("{} ", "Your Username:".yellow());
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut username).unwrap();

        print!("{} ", "Your Password:".yellow());
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut password).unwrap();
    }

    // 3. Target Inputs (Always needed)
    let mut targets_input = String::new();
    print!("{} ", "Targets (e.g. user1,user2):".yellow());
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut targets_input).unwrap();

    let targets: Vec<String> = targets_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    println!(""); 
    
    // 4. Launch
    match launch_browser() {
        Ok(browser) => {
            match InstagramBot::new(&browser) {
                Ok(bot) => {
                    let login_result = if use_saved_session {
                        bot.login_with_session(&saved_session_id)
                    } else {
                        bot.login(username.trim(), password.trim())
                    };

                    if let Err(e) = login_result {
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
