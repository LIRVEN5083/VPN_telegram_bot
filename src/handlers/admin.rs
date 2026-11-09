use crate::{db, handlers::user, HandlerResult};
use sqlx::SqlitePool;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup},
};

// Нижнее меню админа
pub fn admin_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new("👥 Белый список"),
            KeyboardButton::new("📊 Нагрузка VPS"),
        ],
        vec![
            KeyboardButton::new("🔑 Ключи клиентов"),
            KeyboardButton::new("💾 Бэкап сейчас"),
        ],
    ])
    .resize_keyboard(true)
}

pub fn approve_request_keyboard(target_tg_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("✅ Одобрить", format!("approve:{}", target_tg_id)),
        InlineKeyboardButton::callback("❌ Отклонить", format!("reject:{}", target_tg_id)),
    ]])
}

pub async fn handle_admin_message(bot: Bot, msg: Message) -> HandlerResult {
    let text = msg.text().unwrap_or("");

    match text {
        "/start" => {
            bot.send_message(msg.chat.id, "👑 <b>Панель администратора</b>")
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(admin_keyboard())
                .await?;
        }
        "👥 Белый список" => {
            bot.send_message(msg.chat.id, "Здесь будет управление белым списком.")
                .await?;
        }
        "📊 Нагрузка VPS" => {
            bot.send_message(msg.chat.id, "Здесь будет статистика CPU / RAM.")
                .await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "Выберите раздел в меню админа 👇")
                .reply_markup(admin_keyboard())
                .await?;
        }
    }

    Ok(())
}

pub async fn handle_admin_callback(
    bot: Bot,
    q: CallbackQuery,
    pool: SqlitePool,
) -> HandlerResult {
    bot.answer_callback_query(&q.id).await?;

    let Some(data) = &q.data else {
        return Ok(());
    };
    let Some(msg) = &q.message else {
        return Ok(());
    };

    if let Some(id_str) = data.strip_prefix("approve:") {
        let target_id: i64 = id_str.parse()?;

        db::set_whitelist(&pool, target_id, true).await?;

        bot.edit_message_text(
            msg.chat.id,
            msg.id,
            format!("✅ Пользователь <code>{}</code> <b>одобрен</b> и добавлен в белый список!", target_id),
        )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        let _ = bot
            .send_message(
                ChatId(target_id),
                "🎉 <b>Ваша заявка одобрена!</b>\nДоступ к VPN открыт. Используйте меню ниже:",
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(user::user_keyboard())
            .await;
    } else if let Some(id_str) = data.strip_prefix("reject:") {
        let target_id: i64 = id_str.parse()?;

        bot.edit_message_text(
            msg.chat.id,
            msg.id,
            format!("❌ Заявка от пользователя <code>{}</code> <b>отклонена</b>.", target_id),
        )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        let _ = bot
            .send_message(ChatId(target_id), "❌ Ваша заявка на доступ была отклонена.")
            .await;
    }

    Ok(())
}
