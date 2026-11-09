use std::env;
use teloxide::{
    dispatching::{dialogue::{InMemStorage, Dialogue}, UpdateHandler, dialogue},
    prelude::*,
    dptree,
};

mod handlers;
mod config;
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
    // Включаем логирование
    tracing_subscriber::fmt::init();

    // инициализации конфигурации
    let config = Config::load().await;

    let bot = Bot::new(config.bot_token);

    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), config.admin_id])
        .build()
        .dispatch()
        .await;
}

// схема обработки обновлений
fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    // Указываем компилятору брать команды и хэндлеры из модуля handlers
    let command_handler = teloxide::filter_command::<handlers::SimpleCommand, _>()
        .endpoint(handlers::handle_commands);

    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(case![State::AwaitText].endpoint(handlers::handle_await_text))
        .branch(case![State::Start].endpoint(handlers::handle_unknown_message))
        .branch(case![State::ConfirmText(text)].endpoint(handlers::handle_unknown_message));

    let callback_query_handler = Update::filter_callback_query()
        .endpoint(handlers::handle_callback_query);

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
}
