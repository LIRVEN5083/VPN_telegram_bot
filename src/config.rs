use std::env;

pub struct Config {
    pub bot_token: String,
    pub admin_id: u64,
}

impl Config {
    pub fn load() -> Self {
        tracing::info!("--- Initializing VPN bot ---");

        if dotenvy::dotenv().is_err() {
            tracing::warn!(".env file not found, loading variables from system environment.");
        }

        let bot_token = match env::var("BOT_TOKEN") {
            Ok(token) if !token.trim().is_empty() => {
                tracing::info!("✅ BOT_TOKEN: successfully loaded");
                token
            }
            _ => {
                tracing::error!("❌ Startup error: BOT_TOKEN variable is empty or missing!");
                std::process::exit(1);
            }
        };

        let admin_id_str = match env::var("ADMIN_ID") {
            Ok(id) if !id.trim().is_empty() => id,
            _ => {
                tracing::error!("❌ Startup error: ADMIN_ID variable is empty or missing!");
                std::process::exit(1);
            }
        };

        let admin_id: u64 = match admin_id_str.parse() {
            Ok(id) => {
                tracing::info!("✅ ADMIN_ID: successfully loaded (ID: {})", id);
                id
            }
            Err(_) => {
                tracing::error!("❌ Startup error: ADMIN_ID must contain digits only! Provided: '{}'", admin_id_str);
                std::process::exit(1);
            }
        };

        tracing::info!("🚀 All parameters verified. Starting the bot...");

        Config { bot_token, admin_id }
    }
}
