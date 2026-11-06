use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use teloxide::prelude::*;
use tracing::{error, info};

pub mod config;
pub mod handlers;
pub mod xui;

use handlers::{handle_callback_queries, handle_commands, Command};
use xui::XuiClient;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Запуск VPN-бота в модульном режиме...");
    
    dotenv().ok();

    let admin_id: i64 = env::var("ADMIN_TELEGRAM_ID")
        .expect("ADMIN_TELEGRAM_ID не найден в .env")
        .parse()
        .expect("ADMIN_TELEGRAM_ID должен быть числом");

    let xui_url = env::var("XUI_URL").expect("XUI_URL не найден в .env");
    let xui_user = env::var("XUI_LOGIN").expect("XUI_LOGIN не найден в .env");
    let xui_pass = env::var("XUI_PASSWORD").expect("XUI_PASSWORD не найден в .env");
    let server_domain = env::var("SERVER_DOMAIN").unwrap_or_else(|_| "127.0.0.1".to_string());
    
    let xui = Arc::new(XuiClient::new(xui_url));
    if let Err(e) = xui.login(&xui_user, &xui_pass).await {
        error!("Критическая ошибка авторизации в 3x-ui: {}", e);
        panic!("Бот остановлен. Проверьте правильность логина и пароля в .env");
    }
    
    let bot = Bot::from_env();

    let handler = dptree::entry()
        .branch(Update::filter_message().filter_command::<Command>().endpoint(handle_commands))
        .branch(Update::filter_callback_query().endpoint(handle_callback_queries));

    info!("Бот успешно авторизовался везде и начинает слушать Telegram!");

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![xui, admin_id, server_domain])
        .build()
        .dispatch()
        .await;

}
