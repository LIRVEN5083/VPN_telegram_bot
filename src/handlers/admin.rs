use crate::{db, handlers::user, HandlerResult};
use sqlx::SqlitePool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, System};
use teloxide::{
    prelude::*,
    types::{
        InlineKeyboardButton, InlineKeyboardMarkup, InputFile, KeyboardButton, KeyboardMarkup,
    },
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

async fn get_vps_stats_text() -> String {
    let mut sys = System::new_all();
    sys.refresh_cpu_usage();
    tokio::time::sleep(Duration::from_millis(250)).await;
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_usage();
    let cpu_cores = sys.cpus().len();

    let total_ram = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let used_ram = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let ram_percent = if total_ram > 0.0 {
        (used_ram / total_ram) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let mut disk_info = String::from("нет данных");
    for disk in &disks {
        if disk.mount_point() == std::path::Path::new("/") {
            let total_gb = disk.total_space() as f64 / 1024.0 / 1024.0 / 1024.0;
            let avail_gb = disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;
            let used_gb = total_gb - avail_gb;
            disk_info = format!("{:.1} / {:.1} ГБ (свободно {:.1} ГБ)", used_gb, total_gb, avail_gb);
            break;
        }
    }

    let uptime_sec = System::uptime();
    let days = uptime_sec / 86400;
    let hours = (uptime_sec % 86400) / 3600;
    let mins = (uptime_sec % 3600) / 60;

    format!(
        "📊 <b>Состояние VPS:</b>\n\n\
         🖥 <b>CPU ({} ядер):</b> {:.1}%\n\
         🧠 <b>RAM:</b> {:.2} / {:.2} ГБ ({:.1}%)\n\
         💾 <b>Диск (/):</b> {}\n\
         ⏱ <b>Аптайм:</b> {}д {}ч {}м",
        cpu_cores, cpu_usage, used_ram, total_ram, ram_percent, disk_info, days, hours, mins
    )
}

pub async fn handle_admin_message(
    bot: Bot,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    let text = msg.text().unwrap_or("");

    match text {
        "/start" => {
            bot.send_message(msg.chat.id, "👑 <b>Панель администратора</b>")
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(admin_keyboard())
                .await?;
        }
        "👥 Белый список" => {
            let users = db::get_whitelisted_users(&pool).await?;

            if users.is_empty() {
                bot.send_message(msg.chat.id, "📭 Белый список пока пуст.")
                    .await?;
                return Ok(());
            }

            let mut text_out = format!("👥 <b>Белый список ({} чел.):</b>\n\n", users.len());
            let mut buttons: Vec<Vec<InlineKeyboardButton>> = Vec::new();

            for (i, u) in users.iter().enumerate() {
                let uname = u
                    .username
                    .as_deref()
                    .map(|name| format!("@{}", name))
                    .unwrap_or_else(|| "без юзернейма".to_string());

                text_out.push_str(&format!(
                    "{}. {} — <code>{}</code>\n",
                    i + 1,
                    uname,
                    u.tg_id
                ));

                // Кнопка для быстрого исключения из белого списка
                buttons.push(vec![InlineKeyboardButton::callback(
                    format!("🗑 Убрать {} ({})", uname, u.tg_id),
                    format!("revoke:{}", u.tg_id),
                )]);
            }

            bot.send_message(msg.chat.id, text_out)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(buttons))
                .await?;
        }
        "📊 Нагрузка VPS" => {
            let stats = get_vps_stats_text().await;
            bot.send_message(msg.chat.id, stats)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        "🔑 Ключи клиентов" => {
            let users = db::get_whitelisted_users(&pool).await?;

            if users.is_empty() {
                bot.send_message(msg.chat.id, "📭 Активных клиентов в белом списке нет.")
                    .await?;
                return Ok(());
            }

            let mut text_out = String::from("🔑 <b>Ключи и метаданные клиентов:</b>\n\n");

            for u in users {
                let uname = u
                    .username
                    .as_deref()
                    .map(|name| format!("@{}", name))
                    .unwrap_or_else(|| "нет".to_string());

                let uuid_str = u
                    .xui_client_uuid
                    .as_deref()
                    .unwrap_or("<i>не привязан (ожидает 3x-ui)</i>");

                let regen_info = if u.last_key_regen_at == 0 {
                    "не перевыпускался".to_string()
                } else {
                    format!("ts: {}", u.last_key_regen_at)
                };

                text_out.push_str(&format!(
                    "👤 <b>ID:</b> <code>{}</code> ({})\n\
                     🆔 <b>UUID:</b> <code>{}</code>\n\
                     🔄 <b>Перевыпуск:</b> {}\n\
                     📅 <b>В базе с:</b> <code>{}</code>\n\
                     ───────────────\n",
                    u.tg_id, uname, uuid_str, regen_info, u.created_at
                ));
            }

            bot.send_message(msg.chat.id, text_out)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
        "💾 Бэкап сейчас" => {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let temp_path = format!("/tmp/vpn_bot_backup_{}.db", ts);

            if let Err(e) = db::create_backup_file(&pool, &temp_path).await {
                bot.send_message(msg.chat.id, format!("❌ Ошибка создания бэкапа: {}", e))
                    .await?;
                return Ok(());
            }

            let doc = InputFile::file(&temp_path).file_name(format!("backup_{}.db", ts));

            bot.send_document(msg.chat.id, doc)
                .caption("💾 Резервная копия базы данных SQLite")
                .await?;

            // Удаляем временный файл после отправки
            let _ = std::fs::remove_file(&temp_path);
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
            format!(
                "✅ Пользователь <code>{}</code> <b>одобрен</b> и добавлен в белый список!",
                target_id
            ),
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
            format!(
                "❌ Заявка от пользователя <code>{}</code> <b>отклонена</b>.",
                target_id
            ),
        )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        let _ = bot
            .send_message(ChatId(target_id), "❌ Ваша заявка на доступ была отклонена.")
            .await;
    } else if let Some(id_str) = data.strip_prefix("revoke:") {
        let target_id: i64 = id_str.parse()?;

        db::set_whitelist(&pool, target_id, false).await?;

        bot.send_message(
            msg.chat.id,
            format!(
                "🚫 Пользователь <code>{}</code> <b>удален</b> из белого списка.",
                target_id
            ),
        )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;

        let _ = bot
            .send_message(
                ChatId(target_id),
                "⛔️ Ваш доступ к VPN был отозван администратором.",
            )
            .await;
    }

    Ok(())
}
