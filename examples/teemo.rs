use serde_json::Value;
use std::thread;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use teemo::Teemo;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    send_lcu_req().await;
}
async fn send_lcu_req() {
    let (ws_sender, mut ws_recv) = mpsc::channel::<HashMap<String, Value>>(100);
    let mut teemo = Teemo::new();
    teemo.start();
    teemo.start_ws().await;

    let cb_print = Arc::new(|data| {
        println!("----事件数据---- {:?}", data);
    });

    let sender = ws_sender.clone();
    let cb1 = Arc::new(move |data| {
        sender.send(data);
    });
    // 订阅好友列表事件
    teemo.subscribe("/lol-chat/v1/settings", cb1).await;
    teemo
        .subscribe("/lol-lobby/v2/lobby", cb_print.clone())
        .await;
    teemo
        .subscribe("/lol-summoner/v1/current-summoner", cb_print.clone())
        .await;
    teemo
        .subscribe("GetLolLoginV1LoginConnectionState", cb_print.clone())
        .await;
    teemo
        .subscribe("/lol-login/v1/login-connection-state", cb_print.clone())
        .await;
    // teemo.subscribe("OnJsonApiEvent", cb_print.clone()).await;

    // 发送LCU请求
    let summoner = teemo
        .request("GET", "lol-summoner/v1/current-summoner", None)
        .await;
    println!("{:#?}", summoner.get("displayName"));

    thread::sleep(Duration::from_millis(5000));

    println!("取消订阅/lol-chat/v1/settings");
    teemo.unsubscribe("/lol-chat/v1/settings").await;
    
    while let Some(msgs) = ws_recv.recv().await {
        println!("msgs: {:?}", msgs);
    }
}
