use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teloxide::{
    dispatching::dialogue::{Dialogue, InMemStorage},
    dptree,
    prelude::*,
    types::InputFile,
};

mod config;
mod db;
mod handlers;
pub mod xui;

use config::Config;
use xui::XuiClient;

// Описание стейтов бота
#[derive(Clone, Default, Debug)]
pub enum State {
    #[default]
    Start,
    AwaitText,
    ConfirmText(String),
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    let config = Config::load().await;

    let db_pool = db::init_db("bot_data.db").await;

    let xui = XuiClient::new(
        config.xui_url.clone(),
        config.xui_username.clone(),
        config.xui_password.clone(),
        config.server_host.clone(),
    );

    if let Err(e) = xui.login().await {
        tracing::error!("❌ Не удалось войти в 3x-ui (проверь логин/пароль): {}", e);
        std::process::exit(1);
    }
    tracing::info!("✅ 3x-ui: авторизация прошла успешно!");

    let bot = Bot::new(config.bot_token.clone());

    let backup_bot = bot.clone();
    let backup_pool = db_pool.clone();
    let admin_chat_id = ChatId(config.admin_id as i64);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86400));
        interval.tick().await;

        loop {
            interval.tick().await;
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let temp_path = format!("/tmp/auto_backup_{}.db", ts);

            if let Ok(()) = db::create_backup_file(&backup_pool, &temp_path).await {
                let doc = InputFile::file(&temp_path).file_name(format!("auto_backup_{}.db", ts));
                let _ = backup_bot
                    .send_document(admin_chat_id, doc)
                    .caption("🔄 Ежедневный автоматический бэкап базы данных")
                    .await;
                let _ = std::fs::remove_file(&temp_path);
            }
        }
    });

    Dispatcher::builder(bot, handlers::schema())
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            config,
            db_pool,
            xui
        ])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
