# Teemo
> 不要低估迅捷斥候的威力。

本项目用于和英雄联盟游戏客户端进行请求和通信。

# 使用 Usage
本项目适用于：
- Windows
- League Of Legends
## 事件 Event
在`https://127.0.0.1:<PORT>/help`可以查看。

需要注意的是：如果注册了某事件，会触发这个事件及其所有子事件。比如注册`/lol-lobby/v2/lobby`事件，那么`/lol-lobby/v2/lobby/members`也会触发事件回调.
## 示例
[示例](./examples/teemo.rs)

## 对局进行中的事件
对局开始后通过`https://127.0.0.1:2999/swagger/v3/openapi.json`查看接口地址。

可在[官方文档](https://developer.riotgames.com/docs/lol)中搜索`2999`即可找到所有api的功能描述。

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

## LCU主动关闭ws
```bash
word": String("UWpTutcT0rme7Lfw"), "partyId": String("INVID4148783830"), "partyType": String(""), "restrictions": Null, "scarcePositions": Array [], "warnings": Null}, "uri": String("/lol-lobby/v2/lobby")}
thread 'tokio-runtime-worker' panicked at E:\Users\wa\Documents\codes\Teemo\src\utils.rs:148:23:
called `Result::unwrap()` on an `Err` value: Io(Os { code: 10054, kind: ConnectionReset, message: "远程主机强迫关闭了一 个现有的连接。" })
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```