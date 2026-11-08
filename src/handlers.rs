use std::fs::OpenOptions;
use std::io::Write;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
    utils::command::BotCommands,
};
use crate::{MyDialogue, HandlerResult, State};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
pub enum SimpleCommand {
    #[command(description = "Getting started")]
    Start,
    #[command(description = "Save the text to a file")]
    WriteText,
}

pub async fn handle_commands(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    command: SimpleCommand,
) -> HandlerResult {
    match command {
        SimpleCommand::Start => {
            dialogue.reset().await?;
            bot.send_message(msg.chat.id, "Hi! Use the /writetext command to start typing text.").await?;
        }
        SimpleCommand::WriteText => {
            dialogue.update(State::AwaitText).await?;
            bot.send_message(msg.chat.id, "Recording mode is active. Send me the text you want to record:").await?;
        }
    }
    Ok(())
}

// Обработка нажатия на инлайн-кнопку
pub async fn handle_await_text(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
) -> HandlerResult {
    if let Some(text) = msg.text() {
        // Запоминаем текст внутри состояния ConfirmText
        dialogue.update(State::ConfirmText(text.to_string())).await?;

        // Создаем кнопку подтверждения
        let button = InlineKeyboardButton::callback("✅ Confirm action", "confirm_action");
        let keyboard = InlineKeyboardMarkup::new(vec![vec![button]]);

        bot.send_message(msg.chat.id, format!("You entered: \"{}\"\nClick the button below to save it to a file.", text))
            .reply_markup(keyboard)
            .await?;
    } else {
        bot.send_message(msg.chat.id, "Please send just the text.").await?;
    }
    Ok(())
}

// Обработка текста, когда бот находится в состоянии ReceiveText
pub async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
) -> HandlerResult {
    if let Some(data) = q.data {
        if data == "confirm_action" {
            bot.answer_callback_query(q.id).await?;

            // Проверяем, действительно ли у пользователя сейчас состояние ConfirmText и достаем оттуда сохраненный текст
            if let Some(State::ConfirmText(text_to_save)) = dialogue.get().await? {

                // Записываем в файл text.txt
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("text.txt")?;

                writeln!(file, "{}", text_to_save)?;

                if let Some(message) = q.message {
                    // Возвращаем пользователя в начальное состояние
                    dialogue.reset().await?;

                    bot.send_message(message.chat.id, "🎉 The text has been successfully verified and saved to a file: text.txt!")
                        .await?;
                }
            }
        }
    }
    Ok(())
}

// Заглушка
pub async fn handle_unknown_message(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, "Use command /writetext to write your text.").await?;
    Ok(())
}