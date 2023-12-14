use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use std::thread;
use std::time::Duration;
use teemo::Teemo;
use tokio::sync::mpsc::{self, Sender};

#[tokio::main]
async fn main() {
    send_lcu_req().await;
}
async fn send_lcu_req() {
  let (ws_sender, mut ws_recv) = mpsc::channel::<HashMap<String, Value>>(100);
    let mut teemo = Teemo::new();
    teemo.start();
    teemo.start_ws().await;
    
    let sender = ws_sender.clone();
    // let sender_x = sender.clone();
    let cb1 = Arc::new(move |data| {
      sender.send(data);
    });
    // 订阅好友列表事件
    teemo.subscribe("/lol-chat/v1/settings", cb1).await;

    // 订阅好友列表事件
    teemo
        .subscribe("/lol-chat/v1/settings", Arc::new(|data| {
          println!("----teemo./lol-chat/v1/settings---- {:#?}", data);
      }))
        .await;

    // 发送LCU请求
    let mut i = 0;
    while i < 20 {
        i += 1;
        let summoner = teemo
            .request("GET", "lol-summoner/v1/current-summoner", None)
            .await;
        println!("{:#?}", summoner.get("displayName"));

        thread::sleep(Duration::from_millis(500));

        if i == 10 {
            println!("取消订阅/lol-chat/v1/settings");
            teemo.unsubscribe("/lol-chat/v1/settings").await;
        }
    }
    while let Some(msgs) = ws_recv.recv().await {
      println!("msgs: {:?}", msgs);
    }
}
