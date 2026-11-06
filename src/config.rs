use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct XuiResponse<T> {
    pub success: bool,
    pub msg: String,
    pub obj: Option<T>,
}

// Структура самого подключения (инбаунда)
#[derive(Deserialize, Debug, Clone)]
pub struct Inbound {
    pub id: i64,
    pub port: u16,
    pub protocol: String,
    pub remark: String,
    pub settings: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct InboundSettings {
    pub clients: Vec<ClientConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ClientConfig {
    pub id: String,
    pub email: String,

    #[serde(flatten)]
    pub extra: serde_json::Value,
}
