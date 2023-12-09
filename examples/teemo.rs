use teemo::Teemo;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
  send_lcu_req().await;
}

async fn send_lcu_req() {
  let mut teemo = Teemo::new();
  teemo.start().await;
  
  teemo.subscribe("/lol-chat/v1/settings", |data| {
    println!("----teemo./lol-chat/v1/settings---- {:#?}", data.get("eventType"));
  }).await;
  teemo.subscribe("/lol-chat/v1/settings", |data| {
    println!("----teemo./lol-chat/v1/settings 2222---- {:#?}", data.get("uri"));
  }).await;

  // 发送LCU请求
  let mut i = 0;
  while i < 20 {
    i += 1;
    let summoner = teemo.request("GET", "lol-summoner/v1/current-summoner", None).await;
    println!("{:#?}", summoner.get("displayName"));

    thread::sleep(Duration::from_millis(500));

    if i == 10 {
      teemo.close();
    }
  }
}