use async_recursion::async_recursion;
use base64::{engine::general_purpose, Engine as _};
use futures_util::stream::{SplitStream, SplitSink};
use futures_util::{SinkExt, StreamExt};
use http::Request;
use native_tls::TlsConnector;
use reqwest::Method;
use serde_json::Value;
use tokio_util::codec::{Framed, LinesCodec};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    connect_async_tls_with_config, tungstenite::Message, Connector, MaybeTlsStream, WebSocketStream,
};
use url::Url;

mod utils;

#[derive(Debug)]
pub struct Teemo {
    app_token: String,
    app_port: i32,
    url: Url,
    ws_url: Url,
    ws_alive: bool,
    ws_thread: Option<tokio::task::JoinHandle<()>>,
    ws_sender: Option<mpsc::Sender<(String, fn(HashMap<String, Value>))>>,
    ws_tasks: HashMap<String, Vec<fn(HashMap<String, Value>)>>,
    ws_rt: tokio::runtime::Runtime,
}

impl Teemo {
    pub fn new() -> Teemo {
        Teemo {
            app_token: String::from(""),
            app_port: 0,
            url: Url::parse("https://127.0.0.1").unwrap(),
            ws_url: Url::parse("wss://127.0.0.1").unwrap(),
            ws_alive: false,
            ws_thread: None,
            ws_sender: None,
            ws_tasks: HashMap::new(),
            ws_rt: tokio::runtime::Runtime::new().unwrap(),
        }
    }

    pub async fn start(&mut self) {
        self.initialize();
        self.start_ws().await;
    }

    pub fn close(&mut self) {
        self.ws_alive = false;
        self.ws_thread.take().unwrap().abort();
        println!("LCU websocket is closed.");
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
        println!(
            "Attempting to establish connection with LCU websocket: {}",
            self.ws_url.as_str()
        );
        let ws_stream_res =
            connect_async_tls_with_config(request, None, false, Some(connector)).await;

        match ws_stream_res {
            Ok((ws_stream, _ws_res)) => {
                println!("LCU websocket is connected.Captain Teemo on duty.");
                self.ws_alive = true;
                ws_stream
            }
            Err(err) => {
                println!(
                    "LCU websocket connect failed.Teemo will continue to attempt after 500ms.{:#?}",
                    err
                );
                self.ws_alive = false;
                thread::sleep(Duration::from_millis(500));
                self.ws_connector().await
            }
        }
    }

    async fn start_ws(&mut self) {
        let ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>> = self.ws_connector().await;
        let (ws_sender, mut ws_recv) = mpsc::channel::<(String, fn(HashMap<String, Value>))>(100);
        self.ws_sender = Some(ws_sender);

        let (mut writer, mut reader) = ws_stream.split();
        let ws_tasks: Arc<Mutex<HashMap<String, Vec<fn(HashMap<String, Value>)>>>> = Arc::new(Mutex::new(HashMap::new()));
        
        let writer_tasks = Arc::clone(&ws_tasks);
        let reader_tasks = Arc::clone(&ws_tasks);
        let _h = self.ws_rt.spawn(async move {
            while let Some(msgs) = ws_recv.recv().await {
                let delimiter_count = msgs.0.matches("/").count();
                let event_str =
                    "OnJsonApiEvent".to_string() + &msgs.0.replacen("/", "_", delimiter_count);
                let event_str = format!("[5,\"{}\"]", event_str);
                if writer.send(Message::Text(event_str)).await.is_err() {
                    eprintln!("write to client failed");
                    break;
                }
                let mut unlock_tasks = writer_tasks.lock().unwrap();
                match unlock_tasks.get(&msgs.0) {
                    Some(tasks) => {
                        let mut tasks = tasks.clone();
                        tasks.push(msgs.1);
                        unlock_tasks.insert(msgs.0.to_string(), tasks);
                    }
                    None => {
                        let mut tasks: Vec<fn(HashMap<String, Value>)> = Vec::new();
                        tasks.push(msgs.1);
                        unlock_tasks.insert(msgs.0.to_string(), tasks);
                    }
                }
            }
        });
        let handle = self.ws_rt.spawn(async move {
            while let Some(msg) = reader.next().await {
                let msg = msg.unwrap();
                if msg.is_text() || msg.is_binary() {
                    // msg为空时表示ws_stream.send成功
                    if msg.to_string().len() < 1 {
                        continue;
                    }
                    let unlock_tasks = reader_tasks.lock().unwrap();
                    let data: (i32, String, HashMap<String, Value>) =
                        serde_json::from_str(&msg.to_string()).unwrap();
                    let key = data.2.get("uri").unwrap().to_string().replacen("\"", "", 2);
                    // 发送LCU ws结果
                    for task in unlock_tasks.get(&key).unwrap() {
                        task(data.2.clone());
                    }
                }
            }
        });
        self.ws_thread = Some(handle);
    }

    pub async fn subscribe(&mut self, event: &str, callback: fn(HashMap<String, Value>)) {
        self.ws_sender
            .clone()
            .unwrap()
            .send((event.to_string(), callback))
            .await
            .unwrap();
    }
}
