use async_recursion::async_recursion;
use futures_util::StreamExt;
use native_tls::TlsConnector;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fmt, thread};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    connect_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream,
};
use url::Url;
use utils::empty_callback;

mod request;
mod utils;

#[derive(Debug)]
pub struct Teemo {
    pub app_token: String,
    pub app_port: i32,
    pub url: Url,
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
pub type EventCallback = Arc<dyn Fn(HashMap<String, Value>) + Send + Sync>;
pub type EventBody = (EventType, String, EventCallback);
pub type EventTasks = Arc<Mutex<HashMap<String, Vec<EventCallback>>>>;

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
    /// Initialize LCU `remoting-token` and `app-port`.
    ///
    /// After this,`teemo.request` can be used.
    pub fn start(&mut self) {
        self.initialize();
    }
    /// Close all teemo service except `teemo.request`.
    pub fn close(&mut self) {
        self.close_ws();
        println!("LCU websocket is closed.");
    }
    /// Only close websocket with LCU.
    pub fn close_ws(&mut self) {
        self.ws_alive = false;
        for task in self.async_tasks.drain(..) {
            task.abort();
        }
    }
    fn initialize(&mut self) {
        if !cfg!(target_os = "windows") {
            println!("LOL must running at Windows!Teemo will close.");
            return;
        }
        let remote_data = utils::get_lcu_cmd_data();
        if remote_data.len() < 2 {
            println!("LCU is not running.Teemo will try again after 500ms.");
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
    }

    /// Send request to LCU.
    pub async fn request(
        &self,
        method: &str,
        url: &str,
        data: Option<HashMap<String, Value>>,
    ) -> HashMap<String, Value> {
        let full_url = format!("{}{}", self.url, url);

        request::send(method, &full_url, data).await
    }
    /// LOL ingame api.
    ///
    /// You can only connect when the game match is in progress.
    pub async fn live_request(
        &self,
        method: &str,
        url: &str,
        data: Option<HashMap<String, Value>>,
    ) -> HashMap<String, Value> {
        let live_url = "https://127.0.0.1:2999/";
        let full_url = format!("{}{}", live_url, url);

        request::send(method, &full_url, data).await
    }

    #[async_recursion]
    async fn ws_connector(&mut self) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        loop {
            let url = url::Url::parse(self.ws_url.as_str()).unwrap();

            let connector = TlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap();
            let connector = Connector::NativeTls(connector);
            let request = request::create_ws_request(&self.app_token, url);
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
                    break ws_stream;
                }
                Err(err) => {
                    println!(
                        "LCU websocket connect failed.Teemo will continue to attempt after 500ms.{:#?}",
                        err
                    );
                    self.ws_alive = false;
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
            }
        }
    }
    /// Connect to LCU websocket.
    ///
    /// After this,u can subscribe websocket event.
    pub async fn start_ws(&mut self) {
        if !cfg!(target_os = "windows") {
            println!("LOL must running at Windows!Teemo will close.");
            return;
        }
        let ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>> = self.ws_connector().await;
        let (ws_sender, ws_recv) = mpsc::channel::<EventBody>(100);
        self.ws_sender = Some(ws_sender);

        let (writer, reader) = ws_stream.split();
        let ws_tasks: EventTasks = Arc::new(Mutex::new(HashMap::new()));

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
        if !cfg!(target_os = "windows") {
            println!("LOL must running at Windows!Teemo will close.");
            return;
        }
        self.ws_sender
            .clone()
            .unwrap()
            .send((EventType::Subscribe, event.to_string(), callback))
            .await
            .unwrap();
    }

    pub async fn unsubscribe(&mut self, event: &str) {
        if !cfg!(target_os = "windows") {
            println!("LOL must running at Windows!Teemo will close.");
            return;
        }
        self.ws_sender
            .clone()
            .unwrap()
            .send((
                EventType::Unsubscribe,
                event.to_string(),
                Arc::new(empty_callback),
            ))
            .await
            .unwrap();
    }
}
