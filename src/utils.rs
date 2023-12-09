use futures_util::stream::{SplitSink, SplitStream};
use reqwest::{self, Client};
#[cfg(windows)]
use std::{collections::HashMap, os::windows::process::CommandExt, process::Command};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver};
use tokio::{net::TcpStream};
use tokio_tungstenite::{
    tungstenite::Message, MaybeTlsStream, WebSocketStream,
};

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
pub fn create_client() -> Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
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


pub async fn lcu_ws_sender(
    mut writer: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    mut ws_recv: Receiver<(String, fn(HashMap<String, Value>))>,
    writer_tasks: Arc<Mutex<HashMap<String, Vec<fn(HashMap<String, Value>)>>>>,
) {
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
}

pub async fn lcu_ws_receiver(mut reader: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, reader_tasks: Arc<Mutex<HashMap<String, Vec<fn(HashMap<String, Value>)>>>>) {
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
                task(data.2.clone());
            }
        }
    }
}