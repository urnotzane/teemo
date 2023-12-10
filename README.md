# Teemo
> 不要低估迅捷斥候的威力。

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