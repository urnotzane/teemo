use async_recursion::async_recursion;
use futures_util::StreamExt;
use native_tls::TlsConnector;
use reqwest::Method;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{thread, fmt};
use std::time::Duration;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    connect_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream,
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
    ws_sender: Option<mpsc::Sender<EventBody>>,
    async_tasks: Vec<tokio::task::JoinHandle<()>>,
}

#[derive(PartialEq, Clone)]
pub enum EventType {
    Subscribe = 5,
    Unsubscribe = 6,
    Update = 8,
}
impl fmt::Debug for EventType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Subscribe => write!(f, "{}", 5),
            Self::Unsubscribe => write!(f, "{}", 6),
            Self::Update => write!(f, "{}", 8),
        }
    }
}
pub type EventCallback = fn(HashMap<String, Value>);
pub type EventBody = (EventType, String, EventCallback);

impl Teemo {
    pub fn new() -> Teemo {
        Teemo {
            app_token: String::from(""),
            app_port: 0,
            url: Url::parse("https://127.0.0.1").unwrap(),
            ws_url: Url::parse("wss://127.0.0.1").unwrap(),
            ws_alive: false,
            ws_sender: None,
            async_tasks: Vec::new(),
        }
    }

    pub async fn start(&mut self) {
        self.initialize();
        self.start_ws().await;
    }

    pub fn close(&mut self) {
        self.ws_alive = false;
        for task in self.async_tasks.drain(..) {
            task.abort();
        }
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

        let connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let connector = Connector::NativeTls(connector);
        let request = utils::create_ws_request(&self.app_token, url);
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
        let (ws_sender, ws_recv) = mpsc::channel::<EventBody>(100);
        self.ws_sender = Some(ws_sender);

        let (writer, reader) = ws_stream.split();
        let ws_tasks: Arc<Mutex<HashMap<String, Vec<EventCallback>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let writer_tasks = Arc::clone(&ws_tasks);
        let reader_tasks = Arc::clone(&ws_tasks);
        // 记录订阅事件，发送订阅事件至LCU
        let send_task = tokio::spawn(async move {
            utils::lcu_ws_sender(writer, ws_recv, writer_tasks).await;
        });
        // 处理LCU返回的事件，并执行回调函数
        let receive_task = tokio::spawn(async move {
            utils::lcu_ws_receiver(reader, reader_tasks).await;
        });

        self.async_tasks.extend([send_task, receive_task]);
    }

    pub async fn subscribe(&mut self, event: &str, callback: EventCallback) {
        self.ws_sender
            .clone()
            .unwrap()
            .send((EventType::Subscribe, event.to_string(), callback))
            .await
            .unwrap();
    }

    pub async fn unsubscribe(&mut self, event: &str) {
        self.ws_sender
            .clone()
            .unwrap()
            .send((EventType::Unsubscribe, event.to_string(), utils::empty_callback))
            .await
            .unwrap();
    }
}
