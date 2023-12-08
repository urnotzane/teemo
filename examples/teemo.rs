use teemo::Teemo;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
  send_lcu_req().await;
}

async fn send_lcu_req() {
  let mut teemo = Teemo::new();
  teemo.start();
  // 发送LCU请求
  let mut i = 0;
  while i < 10 {
    i += 1;
    let summoner = teemo.request("GET", "lol-summoner/v1/current-summoner", None).await;
    println!("{:#?}", summoner.get("displayName"));

    thread::sleep(Duration::from_millis(500));
  }
  teemo.close();
}