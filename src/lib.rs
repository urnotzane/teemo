use base64::{engine::general_purpose, Engine as _};
use futures_util::{SinkExt, StreamExt};
use http::Request;
use native_tls::TlsConnector;
use reqwest::Method;
use serde_json::Value;
use tokio::net::TcpStream;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use tokio_tungstenite::{connect_async_tls_with_config, tungstenite::Message, Connector, WebSocketStream, MaybeTlsStream};
use url::Url;
use async_recursion::async_recursion;

mod utils;

#[derive(Debug)]
pub struct Teemo {
    app_token: String,
    app_port: i32,
    url: Url,
    ws_url: Url,
    ws_alive: bool,
    ws_handler: Option<tokio::task::JoinHandle<()>>,
}

impl Teemo {
    pub fn new() -> Teemo {
        Teemo {
            app_token: String::from(""),
            app_port: 0,
            url: Url::parse("https://127.0.0.1").unwrap(),
            ws_url: Url::parse("wss://127.0.0.1").unwrap(),
            ws_alive: false,
            ws_handler: None,
        }
    }

    pub async fn start(&mut self) {
        self.initialize();
        self.start_ws().await;
    }

    pub fn close(&mut self) {
        self.ws_alive = false;
        self.ws_handler.take().unwrap().abort();
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

    #[async_recursion]
    async fn ws_connector(&mut self) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let url = url::Url::parse(self.ws_url.as_str()).unwrap();
        let host = url.host_str().expect("Invalid host in WebSocket URL");

        let connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let connector = Connector::NativeTls(connector);

        let auth_base64 = general_purpose::STANDARD.encode(format!("riot:{}", self.app_token));
        println!("auth_base64: {}", auth_base64);
        let request = Request::builder()
            .method("GET")
            .uri(self.ws_url.as_str())
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
            .unwrap();
        println!("Attempting to establish connection with LCU websocket: {}", self.ws_url.as_str());
        let ws_stream_res =
            connect_async_tls_with_config(request, None, false, Some(connector))
                .await;
        // (ws_stream, ws_res)
        match ws_stream_res {
            Ok((ws_stream, _ws_res)) => {
                println!("LCU websocket is connected.Captain Teemo on duty.");
                self.ws_alive = true;
                ws_stream
            },
            Err(err) => {
                println!("LCU websocket connect failed.Teemo will continue to attempt after 500ms.{:#?}", err);
                self.ws_alive = false;
                thread::sleep(Duration::from_millis(500));
                self.ws_connector().await
            }
        }
    } 

    async fn start_ws(&mut self) {
        let mut ws_stream = self.ws_connector().await;

        let handle = tokio::spawn(async move {
            let msgs = "[5,\"OnJsonApiEvent\"]".to_string();

            let _ = ws_stream.send(Message::Text(msgs)).await.unwrap();

            while let Some(msg) = ws_stream.next().await {
                let msg = msg.unwrap();
                if msg.is_text() || msg.is_binary() {
                    // msg为空时表示ws_stream.send成功
                    println!("receive msg: {}", msg);
                }
            }
        });
        self.ws_handler = Some(handle);
    }
}
