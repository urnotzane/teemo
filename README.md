# Teemo
> 不要低估迅捷斥候的威力。

本项目用于和英雄联盟游戏客户端进行请求和通信。

# 使用 Usage
本项目适用于：
- Windows
- League Of Legends
## 事件 Event
在`https://127.0.0.1:<PORT>/help`可以查看
## 示例 Example
```rust
use teemo::Teemo;

#[tokio::main]
async fn main() {
  let mut teemo = Teemo::new();
  teemo.start().await;
  
  // 订阅好友列表事件
  teemo.subscribe("/lol-chat/v1/settings", |data| {
    println!("----teemo./lol-chat/v1/settings---- {:#?}", data);
  }).await;
  teemo.subscribe("/lol-chat/v1/settings", |data| {
    println!("----teemo./lol-chat/v1/settings 2222---- {:#?}", data.get("uri"));
  }).await;

  // 发送LCU请求
  let summoner = teemo.request("GET", "lol-summoner/v1/current-summoner", None).await;
  println!("{:#?}", summoner.get("displayName"));
  
  
  println!("取消订阅/lol-chat/v1/settings");
  teemo.unsubscribe("/lol-chat/v1/settings").await;
}
```

# 其它
## LCU websocket
https://hextechdocs.dev/getting-started-with-the-lcu-websocket/

示例：https://gist.github.com/Pupix/eb662b1b784bb704a1390643738a8c15

## 程序流程
1. 初始化数据；
2. api转发；
3. ws连接；
4. ws检查；
5. ws关闭；
6. 程序关闭。

### 流程错误处理
- 若401，返回至1；
- 若被拒绝，返回至1;
- 若关闭程序，停止所有循环。

## 如何关闭 vscode 的 rust-analyzer 插件的自动类型提示
https://zhuanlan.zhihu.com/p/535828881