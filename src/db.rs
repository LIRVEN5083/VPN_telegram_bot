use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};
use std::str::FromStr;
use std::path::Path;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub tg_id: i64,
    pub username: Option<String>,
    pub is_whitelisted: bool,
    pub xui_client_uuid: Option<String>,
    pub last_request_at: i64,
    pub last_key_regen_at: i64,
    pub created_at: i64,
}

pub async fn init_db(db_path: &str) -> SqlitePool {
    tracing::info!("🗄️ Initializing SQLite database at '{}'...", db_path);

    // Если нету бд, то он её создаст
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))
        .unwrap_or_else(|e| {
            tracing::error!("❌ Invalid database path '{}': {}", db_path, e);
            std::process::exit(1);
        })
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("❌ Failed to connect to SQLite: {}", e);
            std::process::exit(1);
        });

    // Автоматически накатываем таблицы из папки ./migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("❌ Failed to run database migrations: {}", e);
            std::process::exit(1);
        });

    tracing::info!("✅ Database connected and migrations applied!");
    pool
}

// Геттер из бд на юзера по тг айди
pub async fn get_user(pool: &SqlitePool, tg_id: i64) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE tg_id = ?")
        .bind(tg_id)
        .fetch_optional(pool)
        .await
}

pub async fn upsert_user(
    pool: &SqlitePool,
    tg_id: i64,
    username: Option<&str>,
    now_ts: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (tg_id, username, created_at)
         VALUES (?, ?, ?)
         ON CONFLICT(tg_id) DO UPDATE SET username = excluded.username",
    )
        .bind(tg_id)
        .bind(username)
        .bind(now_ts)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_whitelist(
    pool: &SqlitePool,
    tg_id: i64,
    status: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET is_whitelisted = ? WHERE tg_id = ?")
        .bind(status)
        .bind(tg_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_last_request(
    pool: &SqlitePool,
    tg_id: i64,
    now_ts: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_request_at = ? WHERE tg_id = ?")
        .bind(now_ts)
        .bind(tg_id)
        .execute(pool)
        .await?;
    Ok(())
}

// Получить всех пользователей из белого списка
pub async fn get_whitelisted_users(pool: &SqlitePool) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE is_whitelisted = 1 ORDER BY created_at DESC"
    )
        .fetch_all(pool)
        .await
}

// Сделать безопасный слепок БД во временный файл
pub async fn create_backup_file(pool: &SqlitePool, backup_path: &str) -> Result<(), sqlx::Error> {
    // Удаляем старый временный файл, если он остался от прошлого раза
    if Path::new(backup_path).exists() {
        let _ = std::fs::remove_file(backup_path);
    }

    let query = format!("VACUUM INTO '{}'", backup_path);
    sqlx::query(&query).execute(pool).await?;
    Ok(())
}

