use teemo::Teemo;

#[tokio::main]
async fn main() {
  let mut teemo = Teemo::new();
  teemo.start();

  // 发送LCU请求
  let summoner = teemo.request("GET", "lol-summoner/v1/current-summoner", None).await;
  println!("{:#?}", summoner.unwrap());
}