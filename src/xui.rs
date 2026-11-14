use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct XuiClient {
    http: Client,
    base_url: String,
    username: String,
    password: String,
    server_host: String,
    auth_lock: Arc<Mutex<()>>,
}

#[derive(Deserialize)]
struct XuiResponse {
    success: bool,
    msg: Option<String>,
    obj: Option<Value>,
}

impl XuiClient {
    pub fn new(base_url: String, username: String, password: String, server_host: String) -> Self {
        let http = Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(true)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            username,
            password,
            server_host,
            auth_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn login(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _guard = self.auth_lock.lock().await;
        let url = format!("{}/login", self.base_url);

        let resp = self
            .http
            .post(&url)
            .json(&json!({
                "username": self.username,
                "password": self.password
            }))
            .send()
            .await?
            .json::<XuiResponse>()
            .await?;

        if !resp.success {
            return Err(format!("3x-ui login failed: {:?}", resp.msg).into());
        }
        Ok(())
    }

    async fn get_inbounds(&self) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/panel/api/inbounds/list", self.base_url);
        let res = self.http.get(&url).send().await?;

        if !res.status().is_success() {
            self.login().await?;
            let retry = self.http.get(&url).send().await?.json::<XuiResponse>().await?;
            return Ok(retry.obj.and_then(|v| v.as_array().cloned()).unwrap_or_default());
        }

        let parsed: XuiResponse = res.json().await?;
        if !parsed.success {
            self.login().await?;
            let retry = self.http.get(&url).send().await?.json::<XuiResponse>().await?;
            return Ok(retry.obj.and_then(|v| v.as_array().cloned()).unwrap_or_default());
        }

        Ok(parsed.obj.and_then(|v| v.as_array().cloned()).unwrap_or_default())
    }

    pub async fn provision_user_all_inbounds(
        &self,
        tg_id: i64,
        username: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let client_uuid = Uuid::new_v4().to_string();
        let tag = username.unwrap_or("user");

        let inbounds = self.get_inbounds().await?;
        if inbounds.is_empty() {
            return Err("В 3x-ui не найдено ни одного Inbound!".into());
        }

        let add_url = format!("{}/panel/api/inbounds/addClient", self.base_url);

        for inbound in inbounds {
            if !inbound["enable"].as_bool().unwrap_or(true) {
                continue;
            }

            let inbound_id = inbound["id"].as_i64().unwrap_or(0);
            let remark = inbound["remark"].as_str().unwrap_or("vpn");
            let protocol = inbound["protocol"].as_str().unwrap_or("");

            if protocol != "vless" {
                continue;
            }

            let email = format!("{}_{}_{}", tag, tg_id, remark);

            let client_obj = json!({
                "id": client_uuid,
                "flow": "",
                "email": email,
                "limitIp": 0,
                "totalGB": 0,
                "expiryTime": 0,
                "enable": true,
                "tgId": tg_id.to_string(),
                "reset": 0
            });

            let settings_str = json!({
                "clients": [client_obj]
            })
                .to_string();

            let resp = self
                .http
                .post(&add_url)
                .json(&json!({
                    "id": inbound_id,
                    "settings": settings_str
                }))
                .send()
                .await?
                .json::<XuiResponse>()
                .await?;

            if !resp.success {
                tracing::warn!(
                    "Не удалось добавить клиента в inbound {} ({}): {:?}",
                    inbound_id,
                    remark,
                    resp.msg
                );
            }
        }

        Ok(client_uuid)
    }

    pub async fn build_vless_links(
        &self,
        client_uuid: &str,
    ) -> Result<Vec<(String, String)>, Box<dyn std::error::Error + Send + Sync>> {
        let inbounds = self.get_inbounds().await?;
        let mut links = Vec::new();

        for inbound in inbounds {
            if !inbound["enable"].as_bool().unwrap_or(true) {
                continue;
            }
            if inbound["protocol"].as_str() != Some("vless") {
                continue;
            }

            let remark = inbound["remark"].as_str().unwrap_or("VPN").to_string();
            let port = inbound["port"].as_i64().unwrap_or(443);

            let stream_str = inbound["streamSettings"].as_str().unwrap_or("{}");
            let stream: Value = serde_json::from_str(stream_str).unwrap_or(json!({}));

            let network = stream["network"].as_str().unwrap_or("tcp");
            let security = stream["security"].as_str().unwrap_or("none");

            let mut params = vec![
                format!("type={}", network),
                format!("security={}", security),
            ];

            match network {
                "xhttp" | "splithttp" => {
                    let path = stream["xhttpSettings"]["path"]
                        .as_str()
                        .or_else(|| stream["splithttpSettings"]["path"].as_str())
                        .unwrap_or("/");
                    let mode = stream["xhttpSettings"]["mode"]
                        .as_str()
                        .unwrap_or("auto");
                    params.push(format!("path={}", urlencoding_simple(path)));
                    params.push(format!("mode={}", mode));
                }
                "grpc" => {
                    let service_name = stream["grpcSettings"]["serviceName"]
                        .as_str()
                        .unwrap_or("");
                    if !service_name.is_empty() {
                        params.push(format!("serviceName={}", urlencoding_simple(service_name)));
                    }
                }
                "ws" => {
                    let path = stream["wsSettings"]["path"].as_str().unwrap_or("/");
                    params.push(format!("path={}", urlencoding_simple(path)));
                }
                _ => {}
            }

            if security == "reality" {
                let reality = &stream["realitySettings"];
                let settings = &reality["settings"];

                let pbk = settings["publicKey"].as_str().unwrap_or("");
                let fp = settings["fingerprint"].as_str().unwrap_or("chrome");
                let spx = settings["spiderX"].as_str().unwrap_or("/");

                let sni = reality["serverNames"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let sid = reality["shortIds"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                params.push(format!("pbk={}", pbk));
                params.push(format!("fp={}", fp));
                if !sni.is_empty() {
                    params.push(format!("sni={}", sni));
                }
                if !sid.is_empty() {
                    params.push(format!("sid={}", sid));
                }
                if !spx.is_empty() {
                    params.push(format!("spx={}", urlencoding_simple(spx)));
                }
            }

            let link = format!(
                "vless://{}@{}:{}?{}#{}",
                client_uuid,
                self.server_host,
                port,
                params.join("&"),
                urlencoding_simple(&remark)
            );

            links.push((remark, link));
        }

        Ok(links)
    }

    pub async fn delete_client_all_inbounds(
        &self,
        client_uuid: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let inbounds = self.get_inbounds().await?;

        for inbound in inbounds {
            let inbound_id = inbound["id"].as_i64().unwrap_or(0);
            let del_url = format!(
                "{}/panel/api/inbounds/{}/delClient/{}",
                self.base_url, inbound_id, client_uuid
            );
            let _ = self.http.post(&del_url).send().await;
        }

        Ok(())
    }
}

fn urlencoding_simple(s: &str) -> String {
    s.replace('%', "%25")
        .replace('/', "%2F")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
}
