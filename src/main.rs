mod browser;
mod config;
mod instagram;
mod utils;

use std::io::{self, Write};

use colored::*;
use instagram::InstagramBot;
use utils::{clear_terminal, list_profiles, load_profile_session, log_error, log_info, setup_env};

fn read_line(prompt: &str) -> String {
    print!("{} ", prompt.yellow());
    let _ = io::stdout().flush();
    let mut value = String::new();
    if io::stdin().read_line(&mut value).is_err() {
        return String::new();
    }
    value.trim().to_string()
}

#[tokio::main]
async fn main() {
    clear_terminal();
    setup_env();

    println!("{}", "======================================".cyan().bold());
    println!("{}", "       STOV - STORY DOWNLOADER        ".cyan().bold());
    println!(
        "{}",
        "   Reliable capture and media archive  ".white().italic()
    );
    println!("{}", "======================================".cyan().bold());
    println!();

    let profiles = list_profiles().unwrap_or_default();
    let mut username = String::new();
    let mut password = String::new();
    let mut session_id = None;

    if !profiles.is_empty() {
        println!("Saved profiles found:");
        println!("1. Log in with a new account");
        println!("2. Use a saved account");
        if read_line("Select option (1/2):") == "2" {
            for (index, profile) in profiles.iter().enumerate() {
                println!("{}. {}", index + 1, profile);
            }
            if let Ok(index) = read_line("Select profile number:").parse::<usize>() {
                if let Some(profile) = profiles.get(index.saturating_sub(1)) {
                    match load_profile_session(profile) {
                        Ok(value) => {
                            username = profile.clone();
                            session_id = Some(value);
                        }
                        Err(error) => {
                            log_error(&format!("Could not load saved profile: {}", error))
                        }
                    }
                }
            }
        }
    }

    if session_id.is_none() {
        username = read_line("Instagram username:");
        print!("{} ", "Instagram password:".yellow());
        let _ = io::stdout().flush();
        if io::stdin().read_line(&mut password).is_err() {
            log_error("Could not read the password.");
            return;
        }
        password = password.trim_end_matches(['\r', '\n']).to_string();
    }

    let targets: Vec<String> = read_line("Targets (comma-separated usernames):")
        .split(',')
        .map(|target| {
            target
                .trim()
                .trim_start_matches('@')
                .trim_matches('/')
                .to_string()
        })
        .filter(|target| !target.is_empty())
        .collect();
    if targets.is_empty() {
        log_error("No targets were provided.");
        return;
    }

    let browser = match browser::launch_browser() {
        Ok(browser) => browser,
        Err(error) => {
            log_error(&error.to_string());
            return;
        }
    };
    let bot = match InstagramBot::new(&browser) {
        Ok(bot) => bot,
        Err(error) => {
            log_error(&format!("Could not create browser tab: {}", error));
            return;
        }
    };

    let login_result = match session_id {
        Some(session) => bot.login_with_session(&session),
        None => bot.login(&username, &password),
    };
    if let Err(error) = login_result {
        log_error(&format!("Login failed: {}", error));
        return;
    }

    if let Err(error) = bot.process_targets(targets).await {
        log_error(&format!("Scraping failed: {}", error));
        return;
    }
    log_info("Operation completed.");
}
