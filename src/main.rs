use std::env;
use teloxide::{
    dispatching::{dialogue::{InMemStorage, Dialogue}, UpdateHandler, dialogue},
    prelude::*,
    dptree,
};

mod handlers;
mod config;
mod db;

use config::Config;

// Описание стейтов бота
#[derive(Clone, Default, Debug)]
pub enum State {
    #[default]
    Start,                    // Запуск
    AwaitText,                // Бот ждет, пока пользователь напишет текст
    ConfirmText(String),      // Бот получил текст и ждет нажатия кнопки "Подтвердить"
}

// Хранилище состояний конкретного пользователя
type MyDialogue = Dialogue<State, InMemStorage<State>>;
// Это успех выполнения функции
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() {
    // Включаем логирование ну это как библиотека fmt на C++
    tracing_subscriber::fmt::init();

    // инициализации конфигурации
    let config = Config::load().await;
    // Миграция БД если есть то читаем если нету то создаем
    let db_pool = db::init_db("bot_data.db").await;

    let bot = Bot::new(config.bot_token.clone());

    Dispatcher::builder(bot, handlers::schema())
        .dependencies(dptree::deps![
            InMemStorage::<State>::new(),
            config,
            db_pool
        ])
        .build()
        .dispatch()
        .await;
}
