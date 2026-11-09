use crate::{config::Config, db, handlers::admin, HandlerResult};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};
use teloxide::{
    prelude::*,
    types::{KeyboardButton, KeyboardMarkup},
};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// Клавиатура для тех, кого НЕТ в белом списке
fn guest_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![KeyboardButton::new("📝 Подать заявку на доступ")]])
    .resize_keyboard(true)
}

// Клавиатура для тех, кто ЕСТЬ в белом списке
pub fn user_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![KeyboardButton::new("🔑 Мой ключ")],
        vec![
            KeyboardButton::new("🔄 Пересоздать ключ"),
            KeyboardButton::new("📖 Как подключиться?"),
        ],
    ])
    .resize_keyboard(true)
}

pub async fn handle_user_message(
    bot: Bot,
    msg: Message,
    cfg: Config,
    pool: SqlitePool,
) -> HandlerResult {
    let Some(tg_user) = &msg.from() else {
        return Ok(());
    };
    let tg_id = tg_user.id.0 as i64;
    let username = tg_user.username.as_deref();
    let now = now_unix();

    // Сохраняем или обновляем юзера в БД
    db::upsert_user(&pool, tg_id, username, now).await?;
    let user = db::get_user(&pool, tg_id).await?.unwrap();

    let text = msg.text().unwrap_or("");

    // ЕСЛИ ПОЛЬЗОВАТЕЛЯ НЕТ В БЕЛОМ СПИСКЕ
    if !user.is_whitelisted {
        if text == "📝 Подать заявку на доступ" {
            let cooldown = 300; // 5 минут в секундах
            let passed = now - user.last_request_at;

            if passed < cooldown {
                let wait = cooldown - passed;
                bot.send_message(
                    msg.chat.id,
                    format!("⏳ Вы уже отправляли заявку. Повторить можно через {} сек.", wait),
                )
                    .await?;
                return Ok(());
            }

            // Обновляем время заявки в БД
            db::update_last_request(&pool, tg_id, now).await?;

            // Отправляем заявку админу в ЛС
            let mention = username
                .map(|u| format!("@{}", u))
                .unwrap_or_else(|| "без юзернейма".to_string());

            let admin_text = format!(
                "📩 <b>Новая заявка на доступ к VPN!</b>\n\n\
                 👤 Пользователь: {} ({})\n\
                 🆔 ID: <code>{}</code>",
                tg_user.first_name, mention, tg_id
            );

            bot.send_message(ChatId(cfg.admin_id as i64), admin_text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(admin::approve_request_keyboard(tg_id))
                .await?;

            bot.send_message(
                msg.chat.id,
                "✅ Заявка отправлена администратору! Ожидайте уведомления.",
            )
                .await?;
        } else {
            // Любое другое сообщение или /start от заблокированного юзера
            bot.send_message(
                msg.chat.id,
                "⛔️ У вас нет доступа к VPN.\nНажмите кнопку ниже, чтобы подать заявку:",
            )
                .reply_markup(guest_keyboard())
                .await?;
        }
        return Ok(());
    }

    // ЕСЛИ ПОЛЬЗОВАТЕЛЬ ЕСТЬ В БЕЛОМ СПИСКЕ
    match text {
        "/start" => {
            bot.send_message(msg.chat.id, "✅ Доступ активен! Выберите действие:")
                .reply_markup(user_keyboard())
                .await?;
        }
        "🔑 Мой ключ" => {
            bot.send_message(msg.chat.id, "⏳ Скоро здесь будет выдача ключа из 3x-ui...")
                .await?;
        }
        "🔄 Пересоздать ключ" => {
            bot.send_message(msg.chat.id, "⏳ Скоро здесь будет перевыпуск ключа...")
                .await?;
        }
        "📖 Как подключиться?" => {
            bot.send_message(
                msg.chat.id,
                "📖 <b>Инструкция:</b>\n1. Скачайте v2rayNG (Android) или Streisand (iOS).\n2. Нажмите «Мой ключ» и скопируйте ссылку.\n3. Вставьте её в приложение из буфера обмена.",
            )
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "Используйте кнопки меню ниже 👇")
                .reply_markup(user_keyboard())
                .await?;
        }
    }

    Ok(())
}
