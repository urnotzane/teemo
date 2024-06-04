use std::thread;
use std::time::Duration;
use std::sync::Arc;
use teemo::Teemo;

#[tokio::main]
async fn main() {
    send_lcu_req().await;
}
async fn send_lcu_req() {
    let mut teemo = Teemo::new();
    teemo.start();
    teemo.start_ws().await;

    let cb_print = Arc::new(|data| {
        println!("----事件数据---- {:?}", data);
    });
    
    // 订阅事件
    teemo.subscribe("/lol-chat/v1/settings", cb_print.clone()).await;
    // 订阅所有事件
    // teemo.subscribe("OnJsonApiEvent", cb_print.clone()).await;

    // 发送LCU请求
    let summoner = teemo
        .request("GET", "lol-maps/v2/maps", None)
        .await;
    println!("{:?}", summoner);

    thread::sleep(Duration::from_millis(3000));
    println!("取消订阅/lol-chat/v1/settings");
    teemo.unsubscribe("/lol-chat/v1/settings").await;

    let res = teemo.live_request("GET", "liveclientdata/eventdata", None).await;
    println!("游戏中已发生的事件: {:?}", res);
    
    println!("API url: {}", teemo.url);
    
}
