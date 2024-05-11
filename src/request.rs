use std::collections::HashMap;
use http::Request;
use reqwest::{self, Client};
use base64::engine::general_purpose;
use base64::Engine;
use reqwest::Method;
use serde_json::Value;
use url::Url;


/// LOL ingame api.
///
/// You can only connect when the game match is in progress.
pub(crate) async fn send(
    method: &str,
    full_url: &str,
    data: Option<HashMap<String, Value>>,
) -> HashMap<String, Value> {
    let mut error_res: HashMap<String, Value> = HashMap::new();
    error_res.insert("code".to_string(), serde_json::json!(500));
    error_res.insert(
        "message".to_string(),
        serde_json::json!("Teemo request error."),
    );

    let method_byte = method.as_bytes();

    let request = create_client()
        .request(Method::from_bytes(method_byte).unwrap(), full_url)
        .header("Accept", "application/json, text/plain")
        .header("Content-Type", "application/json")
        .json(&data)
        .send()
        .await;
    match request {
        Ok(request) => {
            let response = request.text().await.unwrap();
            match serde_json::from_str(&response) {
                Ok(json_map) => {
                    json_map
                },
                Err(json_error) => {
                    println!("Response json format failed: {:?}", json_error);
                    error_res
                },
            }
        }
        Err(err) => {
            println!("Request api error: {:?}", err);
            error_res
        }
    }
}

pub(crate) fn create_client() -> Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .unwrap()
}
pub(crate) fn create_ws_request(token: &str, url: Url) -> Request<()> {
    let auth_base64 = general_purpose::STANDARD.encode(format!("riot:{}", token));
    let host = url.host_str().expect("Invalid host in WebSocket URL");
    Request::builder()
        .method("GET")
        .uri(url.as_str())
        // LCU API认证
        .header("Authorization", format!("Basic {}", auth_base64))
        .header("Host", host)
        .header("Upgrade", "websocket")
        .header("Connection", "upgrade")
        .header("Sec-Websocket-Key", "lcu")
        .header("Sec-Websocket-Version", "13")
        .header("Accept", "application/json, text/plain")
        .header("Content-Type", "application/json")
        .body(())
        .unwrap()
}