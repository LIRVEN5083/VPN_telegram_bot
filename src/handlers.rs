use crate::config::InboundSettings;
use crate::xui::XuiClient;
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode},
    utils::command::BotCommands,
};
use tracing::{error, warn};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Доступные команды:")]
pub enum Command {
    #[command(description = "Запустить бота")]
    Start,
    #[command(description = "Управление моим VPN")]
    Vpn,
}

pub async fn handle_commands(
    bot: Bot,
    msg: Message,
    cmd: Command,
    admin_id: i64,
    xui: Arc<XuiClient>,
) -> ResponseResult<()> {
    if msg.chat.id.0 != admin_id {
        bot.send_message(msg.chat.id, "❌ Доступ запрещен.").await?;
        return Ok(());
    }


    match cmd {
        Command::Start => {
            bot.send_message(
                msg.chat.id,
                "👋 Привет, админ! Я твой личный менеджер VPN.\nИспользуй команду /vpn для управления ключами.",
            )
                .await?;
        }
        Command::Vpn => {
            bot.send_message(msg.chat.id, "🔍 Запрашиваю данные из 3x-ui...").await?;

            match xui.get_inbounds().await {
                Ok(inbounds) => {
                    if inbounds.is_empty() {
                        bot.send_message(msg.chat.id, "📭 В панели пока нет созданных подключений.").await?;
                        return Ok(());
                    }

                    if let Some(inbound) = inbounds.first() {
                        if let Ok(settings) = serde_json::from_str::<InboundSettings>(&inbound.settings) {
                            if let Some(client) = settings.clients.first() {

                                let callback_data = format!("regen:{}:{}", inbound.id, client.id);
                                let keyboard = InlineKeyboardMarkup::new(vec![vec![
                                    InlineKeyboardButton::callback("🔄 Перегенерировать ссылку", callback_data)
                                ]]);

                                let info_text = format!(
                                    "👤 **Ваш VPN профиль**\n\n\
                                     🔹 **Протокол:** {}\n\
                                     🔹 **Имя:** `{}`\n\
                                     🔹 **Текущий UUID:** `{}`\n\
                                     🔹 **Порт:** `{}`\n\n\
                                     ⚡ Нажмите кнопку ниже, чтобы сбросить UUID и получить новую ссылку.",
                                    inbound.protocol, client.email, client.id, inbound.port
                                );

                                bot.send_message(msg.chat.id, info_text)
                                    .parse_mode(ParseMode::MarkdownV2)
                                    .reply_markup(keyboard)
                                    .await?;
                                return Ok(());
                            }
                        }
                    }
                    bot.send_message(msg.chat.id, "⚠️ Не удалось распарсить пользователей внутри инбаунда.").await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("❌ Ошибка API: {}", e)).await?;
                }
            }
        }
    }
    Ok(())
}

pub async fn handle_callback_queries(
    bot: Bot,
    q: CallbackQuery,
    xui: Arc<XuiClient>,
    admin_id: i64,
    server_domain: String,
) -> ResponseResult<()> {
    if q.from.id.0 as i64 != admin_id {
        return Ok(());
    }


    if let Some(data) = q.data {
        if data.starts_with("regen:") {
            let parts: Vec<&str> = data.split(':').collect();
            if parts.len() < 3 { return Ok(()); }

            let inbound_id: i64 = parts[1].parse().unwrap_or(0);
            let old_uuid = parts[2];

            if let Some(msg) = q.message {
                bot.send_message(msg.chat.id, "🔄 Генерирую новый ключ и обновляю панель...").await?;

                if let Ok(inbounds) = xui.get_inbounds().await {
                    if let Some(inbound) = inbounds.iter().find(|i| i.id == inbound_id) {
                        if let Ok(mut settings) = serde_json::from_str::<InboundSettings>(&inbound.settings) {
                            if let Some(client) = settings.clients.iter_mut().find(|c| c.id == old_uuid) {

                                let new_uuid = uuid::Uuid::new_v4().to_string();
                                let target_uuid = client.id.clone();
                                client.id = new_uuid.clone();

                                let updated_client_json = serde_json::json!({
                                    "id": client.id,
                                    "email": client.email,
                                    "flow": client.extra.get("flow").unwrap_or(&serde_json::Value::Null),
                                    "subId": client.extra.get("subId").unwrap_or(&serde_json::Value::Null),
                                });

                                match xui.update_client(inbound_id, &target_uuid, updated_client_json).await {
                                    Ok(_) => {
                                        let vless_link = format!(
                                            "vless://{}@{}:{}?type=tcp&security=none#{}",
                                            new_uuid, server_domain, inbound.port, inbound.remark
                                        );

                                        let success_text = format!(
                                            "✅ **Успешно перегенерировано!**\n\n\
                                             🔑 **Новый UUID:** `{}`\n\n\
                                             📋 **Ваша новая ссылка (нажмите, чтобы скопировать):**\n`{}`",
                                            new_uuid, vless_link
                                        );

                                        bot.send_message(msg.chat.id, success_text)
                                            .parse_mode(ParseMode::MarkdownV2)
                                            .await?;
                                    }
                                    Err(e) => {
                                        bot.send_message(msg.chat.id, format!("❌ Не удалось обновить в панели: {}", e)).await?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}
