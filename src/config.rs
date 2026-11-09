use std::env;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub admin_id: u64,
    pub xui_url: String,
    pub xui_login: String,
    pub xui_password: String,
}

impl Config {
    pub async fn load() -> Self {
        tracing::info!("--- Initializing VPN bot ---");

        if dotenvy::dotenv().is_err() {
            tracing::warn!(".env file not found, loading variables from system environment.");
        }

        let bot_token = Self::get_required_var("BOT_TOKEN");
        tracing::info!("✅ BOT_TOKEN: successfully loaded");

        let admin_id_str = Self::get_required_var("ADMIN_ID");
        let admin_id: u64 = match admin_id_str.trim().parse() {
            Ok(id) => {
                tracing::info!("✅ ADMIN_ID: successfully loaded (ID: {})", id);
                id
            }
            Err(_) => {
                tracing::error!(
                    "❌ Startup error: ADMIN_ID must contain digits only! Provided: '{}'",
                    admin_id_str
                );
                std::process::exit(1);
            }
        };

        let raw_xui_url = Self::get_required_var("XUI_URL");
        let xui_url = raw_xui_url.trim().trim_end_matches('/').to_string();
        tracing::info!("✅ XUI_URL: successfully loaded ({})", xui_url);

        // Отправляем что-то типо curl и проверяем вообще существует ли адресс
        Self::verify_xui_url(&xui_url).await;

        let xui_login = Self::get_required_var("XUI_LOGIN");
        tracing::info!("✅ XUI_LOGIN: successfully loaded ({})", xui_login);

        let xui_password = Self::get_required_var("XUI_PASSWORD");
        tracing::info!("✅ XUI_PASSWORD: successfully loaded (********)");

        tracing::info!("🚀 All parameters verified. Starting the bot...");

        Config {
            bot_token,
            admin_id,
            xui_url,
            xui_login,
            xui_password,
        }
    }

    async fn verify_xui_url(url: &str) {
        tracing::info!("🔍 Checking connection to 3x-ui panel...");

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!("❌ Failed to build HTTP client: {}", e);
                std::process::exit(1);
            });

        match client.get(url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    tracing::info!("✅ 3x-ui panel is reachable! (Status: {})", response.status());
                } else {
                    tracing::error!(
                        "❌ Startup error: 3x-ui returned status {} for '{}'. Check your webBasePath!",
                        response.status(),
                        url
                    );
                    std::process::exit(1);
                }
            }
            Err(e) => {
                tracing::error!(
                    "❌ Startup error: Could not connect to 3x-ui at '{}'. Details: {}",
                    url,
                    e
                );
                std::process::exit(1);
            }
        }
    }

    fn get_required_var(name: &str) -> String {
        match env::var(name) {
            Ok(val) if !val.trim().is_empty() => val.trim().to_string(),
            _ => {
                tracing::error!("❌ Startup error: {} variable is empty or missing!", name);
                std::process::exit(1);
            }
        }
    }
}
