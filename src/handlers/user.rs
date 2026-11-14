use crate::{config::Config, db, handlers::admin, xui::XuiClient, HandlerResult};
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

fn guest_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![vec![KeyboardButton::new("📝 Подать заявку на доступ")]])
        .resize_keyboard(true)
}

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
    xui: XuiClient,
) -> HandlerResult {
    let Some(tg_user) = &msg.from() else {
        return Ok(());
    };
    let tg_id = tg_user.id.0 as i64;
    let username = tg_user.username.as_deref();
    let now = now_unix();

    db::upsert_user(&pool, tg_id, username, now).await?;
    let user = db::get_user(&pool, tg_id).await?.unwrap();

    let text = msg.text().unwrap_or("");


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

            db::update_last_request(&pool, tg_id, now).await?;

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
            bot.send_message(
                msg.chat.id,
                "⛔️ У вас нет доступа к VPN.\nНажмите кнопку ниже, чтобы подать заявку:",
            )
                .reply_markup(guest_keyboard())
                .await?;
        }
        return Ok(());
    }

    match text {
        "/start" => {
            bot.send_message(msg.chat.id, "✅ Доступ активен! Выберите действие:")
                .reply_markup(user_keyboard())
                .await?;
        }
        "🔑 Мой ключ" => {
            let uuid = match &user.xui_client_uuid {
                Some(existing_uuid) => existing_uuid.clone(),
                None => {
                    bot.send_message(msg.chat.id, "⏳ Создаю доступ во всех маршрутах...")
                        .await?;

                    match xui.provision_user_all_inbounds(tg_id, username).await {
                        Ok(new_uuid) => {
                            db::save_user_uuid(&pool, tg_id, &new_uuid, now).await?;
                            new_uuid
                        }
                        Err(e) => {
                            tracing::error!("Ошибка создания ключа в 3x-ui: {}", e);
                            bot.send_message(
                                msg.chat.id,
                                "❌ Ошибка при обращении к VPN-серверу. Сообщите администратору.",
                            )
                                .await?;
                            return Ok(());
                        }
                    }
                }
            };

            match xui.build_vless_links(&uuid).await {
                Ok(links) if !links.is_empty() => {
                    let mut reply = String::from(
                        "🔑 <b>Ваши ключи доступа (Бессрочно ∞):</b>\n\
                         <i>Нажмите на любой ключ, чтобы скопировать его:</i>\n\n",
                    );

                    for (remark, link) in links {
                        reply.push_str(&format!(
                            "🌐 <b>{}</b>\n<code>{}</code>\n\n",
                            remark, link
                        ));
                    }

                    bot.send_message(msg.chat.id, reply)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .await?;
                }
                Ok(_) => {
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ На сервере пока нет активных маршрутов. Обратитесь к администратору.",
                    )
                        .await?;
                }
                Err(e) => {
                    tracing::error!("Ошибка сборки vless ссылок: {}", e);
                    bot.send_message(
                        msg.chat.id,
                        "❌ Не удалось получить ключи с сервера. Попробуйте позже.",
                    )
                        .await?;
                }
            }
        }
        "🔄 Пересоздать ключ" => {
            let cooldown = 86400; // 24 часа в секундах
            let passed = now - user.last_key_regen_at;

            if user.xui_client_uuid.is_some() && passed < cooldown {
                let left = cooldown - passed;
                let hours = left / 3600;
                let mins = (left % 3600) / 60;
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "⏳ Перевыпускать ключ можно раз в сутки.\nПовторить можно через <b>{} ч. {} мин.</b>",
                        hours, mins
                    ),
                )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                return Ok(());
            }

            bot.send_message(msg.chat.id, "🔄 Аннулирую старые ключи и выпускаю новые...")
                .await?;

            if let Some(old_uuid) = &user.xui_client_uuid {
                let _ = xui.delete_client_all_inbounds(old_uuid).await;
            }

            match xui.provision_user_all_inbounds(tg_id, username).await {
                Ok(new_uuid) => {
                    db::save_user_uuid(&pool, tg_id, &new_uuid, now).await?;

                    if let Ok(links) = xui.build_vless_links(&new_uuid).await {
                        let mut reply = String::from(
                            "✅ <b>Ключи пересозданы!</b>\n\
                             <i>Старые ключи больше не работают. Ваши новые ключи:</i>\n\n",
                        );

                        for (remark, link) in links {
                            reply.push_str(&format!(
                                "🌐 <b>{}</b>\n<code>{}</code>\n\n",
                                remark, link
                            ));
                        }

                        bot.send_message(msg.chat.id, reply)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .await?;
                    }
                }
                Err(e) => {
                    tracing::error!("Ошибка перевыпуска ключа: {}", e);
                    bot.send_message(
                        msg.chat.id,
                        "❌ Не удалось пересоздать ключ. Попробуйте позже.",
                    )
                        .await?;
                }
            }
        }
        "📖 Как подключиться?" => {
            bot.send_message(
                msg.chat.id,
                "📖 <b>Инструкция по подключению:</b>\n\n\
                 1. Скачайте приложение:\n\
                 • <b>Android:</b> v2rayNG или Hiddify\n\
                 • <b>iOS:</b> Streisand, V2Box или Hiddify\n\
                 • <b>Windows / macOS:</b> Hiddify или Nekoray\n\n\
                 2. Нажмите кнопку <b>«🔑 Мой ключ»</b> и тапните по нужному ключу, чтобы скопировать.\n\
                 3. В приложении нажмите <b>«+» → «Импорт из буфера обмена»</b>.\n\
                 4. Выберите добавленный профиль и нажмите кнопку подключения.",
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
