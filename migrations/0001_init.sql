CREATE TABLE IF NOT EXISTS users (
    tg_id INTEGER PRIMARY KEY NOT NULL,
    username TEXT,
    is_whitelisted BOOLEAN NOT NULL DEFAULT 0,
    xui_client_uuid TEXT,
    last_request_at INTEGER NOT NULL DEFAULT 0,
    last_key_regen_at INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
