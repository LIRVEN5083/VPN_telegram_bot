use crate::config::{Inbound, XuiResponse};
use reqwest::Client;
use tracing::info;

pub struct XuiClient {
    http: Client,
    base_url: String,
}

impl XuiClient {
    pub fn new(base_url: String) -> Self {
        let http = Client::builder()
            .cookie_store(true)
            .build()
            .expect("Не удалось создать HTTP клиент для 3x-ui");
        Self { http, base_url }
    }
    
    pub async fn login(&self, username: &str, password: &str) -> Result<(), String> {
        let url = format!("{}/login", self.base_url);
        let params = [("username", username), ("password", password)];

        let res = self.http.post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Ошибка сети при логине: {}", e))?;

        let body: XuiResponse<serde_json::Value> = res.json()
            .await
            .map_err(|e| format!("Ошибка парсинга ответа логина: {}", e))?;

        if body.success {
            info!("Успешная авторизация в панели 3x-ui!");
            Ok(())
        } else {
            Err(format!("Панель отклонила логин: {}", body.msg))
        }
    }
    
    pub async fn get_inbounds(&self) -> Result<Vec<Inbound>, String> {
        let url = format!("{}/panel/api/inbounds/list", self.base_url);
        let res = self.http.get(&url)
            .send()
            .await
            .map_err(|e| format!("Ошибка сети при запросе инбаундов: {}", e))?;

        let body: XuiResponse<Vec<Inbound>> = res.json()
            .await
            .map_err(|e| format!("Ошибка парсинга списка инбаундов: {}", e))?;

        if body.success {
            Ok(body.obj.unwrap_or_default())
        } else {
            Err(format!("Ошибка получения данных: {}", body.msg))
        }
    }
    
    pub async fn update_client(&self, inbound_id: i64, client_uuid: &str, updated_client_json: serde_json::Value) -> Result<(), String> {
        let url = format!("{}/panel/api/inbounds/updateClient/{}", self.base_url, client_uuid);

        let json_body = serde_json::json!({
            "id": inbound_id,
            "settings": updated_client_json.to_string()
        });

        let res = self.http.post(&url)
            .json(&json_body)
            .send()
            .await
            .map_err(|e| format!("Ошибка сети при обновлении клиента: {}", e))?;

        let body: XuiResponse<serde_json::Value> = res.json()
            .await
            .map_err(|e| format!("Ошибка парсинга ответа обновления: {}", e))?;

        if body.success {
            Ok(())
        } else {
            Err(format!("Панель не смогла обновить клиента: {}", body.msg))
        }
    }
}
