use crate::{EventBody, EventCallback, EventType, EventTasks};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{collections::HashMap, process::Command};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

#[test]
fn it_works() {
    let mut assert_res = HashMap::new();
    assert_res.insert(
        "remoting-auth-token".to_string(),
        "_BkC3zoDF6600gmlQdUs6w".to_string(),
    );
    assert_res.insert("app-port".to_string(), "58929".to_string());
    assert_eq!(
        format_lcu_data(
            "\"--remoting-auth-token=_BkC3zoDF6600gmlQdUs6w\" \"--app-port=58929\"".to_string(),
        ),
        assert_res
    );
    // For `fn format_event_type`
    assert_eq!(format_event_type("/lol-summoner", EventType::Subscribe), "[5,\"OnJsonApiEvent_lol-summoner\"]");
    assert_eq!(format_event_type("Exit", EventType::Subscribe), "[5,\"Exit\"]");
    // For `fn revert_event_type`
    assert_eq!(revert_event_type("OnJsonApiEvent_lol-lobby_v2_lobby".to_string()), "/lol-lobby/v2/lobby".to_string());
    assert_eq!(revert_event_type("OnJsonApiEvent".to_string()), "OnJsonApiEvent".to_string());
    assert_eq!(revert_event_type("OnServiceProxyAsyncEvent".to_string()), "OnServiceProxyAsyncEvent".to_string());
}

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
#[cfg(not(windows))]
pub fn execute_command(cmd_str: &str) -> String {
    let output = Command::new(cmd_str)
        .arg("-l")
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout
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

pub fn format_event_type(event: &str, event_type: EventType) -> String {
    let delimiter_count = event.matches("/").count();
    let event_str = match delimiter_count {
        0 => event.to_string(),
        _ => "OnJsonApiEvent".to_string() + &event.replacen("/", "_", delimiter_count),
    };

    format!("[{:?},\"{}\"]", event_type, event_str)
}

pub fn revert_event_type(event: String) -> String {
    let delimiter_count = event.matches("_").count();
    let event_uri = event.replacen("OnJsonApiEvent", "", 1)
        .replacen("_", "/", delimiter_count);
    
    if event_uri.len() < 1 {
        return event;
    }
    event_uri
}

pub(crate) async fn lcu_ws_sender(
    mut writer: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    mut ws_recv: Receiver<EventBody>,
    writer_tasks: EventTasks,
) {
    while let Some(msgs) = ws_recv.recv().await {
        let (event_type, event, event_callback) = msgs;
        let event_str = format_event_type(&event, event_type.clone());

        if writer.send(Message::Text(event_str)).await.is_err() {
            println!("Event '{}' subscribe failed.", event);
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

pub(crate) async fn lcu_ws_receiver(
    mut reader: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    reader_tasks: EventTasks,
) {
    while let Some(msg) = reader.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                println!("LCU websocket connection failed: {:?}", err);
                break;
            },
        };
        if msg.is_text() || msg.is_binary() {
            // msg为空时表示订阅LCU事件成功
            if msg.to_string().len() < 1 {
                continue;
            }
            println!("Received websocket message: {:?}", msg);
            let unlock_tasks = reader_tasks.lock().unwrap();
            let data: (i32, String, HashMap<String, Value>) =
                serde_json::from_str(&msg.to_string()).unwrap();
            let key = revert_event_type(data.1);
            // 执行订阅的事件回调
            match unlock_tasks.get(&key) {
                Some(callbacks) => {
                    for cb in callbacks {
                        let callback = cb.clone();
                        (*callback)(data.2.clone());
                    }
                }
                None => {
                    println!("Event {} has no callbacks.", key);
                }
            }
        }
    }
}

pub(crate) fn empty_callback(_data: HashMap<String, Value>) {}
