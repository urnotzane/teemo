use base64::Engine;
use base64::engine::general_purpose;
use futures_util::stream::{SplitSink, SplitStream};
use reqwest::{self, Client};
use http::Request;
use url::Url;
#[cfg(windows)]
use std::{collections::HashMap, os::windows::process::CommandExt, process::Command};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Receiver;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::Message, MaybeTlsStream, WebSocketStream,
};

use crate::{EventCallback, EventBody, EventType};

#[cfg(windows)]
pub fn execute_command(cmd_str: &str) -> String {
    let output = Command::new("cmd")
        // 运行cmd时隐藏窗口
        .creation_flags(0x08000000)
        .args(["/C", cmd_str])
        .output()
        .expect("failed to execute process");

    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn get_lcu_cmd_data() -> HashMap<String, String> {
    let data_str = execute_command("wmic PROCESS WHERE name='LeagueClientUx.exe' GET commandline");
    format_lcu_data(data_str)
}

fn format_lcu_data(data_str: String) -> HashMap<String, String> {
    let mut data_map = HashMap::new();
    let col: Vec<_> = data_str.split("\"--").collect();
    if col.len() < 2 {
        return data_map;
    }
    for item in col {
        let temp: Vec<_> = item.split("=").collect();
        if temp.len() < 2 {
            continue;
        }
        data_map.insert(
            temp[0].to_string(),
            temp[1].replace("\"", "").trim().to_string(),
        );
    }
    data_map
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

#[test]
fn it_works() {
    let result = format_lcu_data(
        "\"--remoting-auth-token=_BkC3zoDF6600gmlQdUs6w\" \"--app-port=58929\"".to_string(),
    );
    let mut assert_res = HashMap::new();
    assert_res.insert(
        "remoting-auth-token".to_string(),
        "_BkC3zoDF6600gmlQdUs6w".to_string(),
    );
    assert_res.insert("app-port".to_string(), "58929".to_string());
    assert_eq!(result, assert_res);
}

pub fn format_event_type(event:&str, event_type:EventType) -> String {
    let delimiter_count = event.matches("/").count();
    let event_str =
        "OnJsonApiEvent".to_string() + &event.replacen("/", "_", delimiter_count);

    format!("[{:?},\"{}\"]", event_type, event_str)
}

pub(crate) async fn lcu_ws_sender(
    mut writer: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    mut ws_recv: Receiver<EventBody>,
    writer_tasks: Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
) {
    while let Some(msgs) = ws_recv.recv().await {
        let (event_type, event, event_callback) = msgs;
        let event_str = format_event_type(&event, event_type.clone());
        
        if writer.send(Message::Text(event_str)).await.is_err() {
            eprintln!("write to client failed");
            break;
        }
        let mut unlock_tasks = writer_tasks.lock().unwrap();
        // 取消订阅
        if event_type == EventType::Unsubscribe {
            unlock_tasks.remove(&event);
            continue;
        }
        match unlock_tasks.get(&event) {
            // 新增该型事件回调
            Some(tasks) => {
                let mut tasks = tasks.clone();
                tasks.push(event_callback);
                unlock_tasks.insert(event.to_string(), tasks);
            }
            // 新增订阅
            None => {
                let mut tasks: Vec<EventCallback> = Vec::new();
                tasks.push(event_callback);
                unlock_tasks.insert(event.to_string(), tasks);
            }
        }
    }
}

pub(crate) async fn lcu_ws_receiver(mut reader: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, reader_tasks: Arc<Mutex<HashMap<String, Vec<EventCallback>>>>) {
    while let Some(msg) = reader.next().await {
        let msg = msg.unwrap();
        if msg.is_text() || msg.is_binary() {
            // msg为空时表示订阅LCU事件成功
            if msg.to_string().len() < 1 {
                continue;
            }
            let unlock_tasks = reader_tasks.lock().unwrap();
            let data: (i32, String, HashMap<String, Value>) =
                serde_json::from_str(&msg.to_string()).unwrap();
            let key = data.2.get("uri").unwrap().to_string().replacen("\"", "", 2);
            // 执行订阅的事件回调
            for task in unlock_tasks.get(&key).unwrap() {
                let callback = task.clone();
                (*callback)(data.2.clone());
            }
        }
    }
}

pub(crate) fn empty_callback(_data: HashMap<String, Value>) {}
