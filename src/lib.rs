
use base64::{Engine as _, engine::general_purpose};
use http::Request;
use native_tls::TlsConnector;
use reqwest::Method;
use serde_json::Value;
use tokio_tungstenite::{Connector, connect_async_tls_with_config, tungstenite::Message};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use url::Url;
use futures_util::{StreamExt, SinkExt};

mod utils;

#[derive(Debug)]
pub struct Teemo {
    app_token: String,
    app_port: i32,
    url: Url,
    ws_url: Url,
    closed: bool,
}

impl Teemo {
    pub fn new() -> Teemo {
        Teemo {
            app_token: String::from(""),
            app_port: 0,
            url: Url::parse("https://127.0.0.1").unwrap(),
            ws_url: Url::parse("wss://127.0.0.1").unwrap(),
            closed: false,
        }
    }

    pub fn start(&mut self) {
        self.closed = false;
        self.initialize();
    }

    pub fn close(&mut self) {
        self.closed = true;
        println!("Armed and ready.");
    }

    fn initialize(&mut self) {
        let remote_data = utils::get_lcu_cmd_data();
        if remote_data.len() < 2 {
            println!("LCU is not running.");
            thread::sleep(Duration::from_millis(500));
            self.initialize();
            return;
        }

        self.app_token = remote_data.get("remoting-auth-token").unwrap().to_owned();
        self.app_port = remote_data
            .get("app-port")
            .unwrap()
            .to_owned()
            .parse::<i32>()
            .unwrap();
        self.url =
            Url::parse(&("https://127.0.0.1:".to_string() + &self.app_port.to_string())).unwrap();
        self.ws_url =
            Url::parse(&("wss://127.0.0.1:".to_string() + &self.app_port.to_string())).unwrap();

        println!("Teemo has finished initializing.");
        println!("LCU is running on {}, token: {}", self.url, self.app_token);
    }

    /// 用来发送LCU请求
    pub async fn request(
        &self,
        method: &str,
        url: &str,
        data: Option<HashMap<String, Value>>,
    ) -> HashMap<String, Value> {
        let mut error_res: HashMap<String, Value> = HashMap::new();
        error_res.insert("code".to_string(), serde_json::json!(500));
        error_res.insert(
            "message".to_string(),
            serde_json::json!("LCU service error."),
        );

        if self.app_token.len() < 1 {
            return error_res;
        }

        let method_byte = method.as_bytes();
        let full_url = format!("{}{}", self.url, url);

        let request = utils::create_client()
            .request(Method::from_bytes(method_byte).unwrap(), full_url)
            .basic_auth("riot", Some(&self.app_token))
            .header("Accept", "application/json, text/plain")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;

        match request {
            Ok(request) => {
                let response = request.text().await.unwrap();
                serde_json::from_str(&response).unwrap()
            }
            Err(err) => {
                println!("LCU service error: {:?}", err);
                error_res
            }
        }
    }

    async fn start_ws(&self) {
        let base_url = &format!("{}:{}", self.ws_url, self.app_port);
    
        let url = url::Url::parse(base_url).unwrap();
        let host = url.host_str().expect("Invalid host in WebSocket URL");
    
        let connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let connector = Connector::NativeTls(connector);

        let auth_base64 = general_purpose::STANDARD.encode(format!("riot:{}", self.app_token));
        let request = Request::builder()
            .method("GET")
            .uri(base_url)
            // LCU API认证
            .header("Authorization", format!("Basic {}", auth_base64))
            .header("Host", host)
            .header("Upgrade", "websocket")
            .header("Connection", "upgrade")
            .header("Sec-Websocket-Key", "lcu")
            .header("Sec-Websocket-Version", "13")
            .body(())
            .unwrap();
        println!("------开始连接{}------", base_url.as_str());
        let (mut ws_stream, ws_res) = connect_async_tls_with_config(
            request, None, false, Some(connector))
            .await.expect("连接失败");
        
        println!("-----连接成功：{:#?}------", ws_res);
        
        let msgs = "[5,\"OnJsonApiEvent\"]".to_string();
    
        let _ = ws_stream.send(Message::Text(msgs)).await.unwrap();
    
        while let Some(msg) = ws_stream.next().await {
            let msg = msg.unwrap();
            if msg.is_text() || msg.is_binary() {
                // msg为空时表示ws_stream.send成功
                println!("receive msg: {}", msg);
            }
        }
    }
}
