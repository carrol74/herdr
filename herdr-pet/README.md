# herdr-pet

`herdr-pet` 是一个轻量桌面宠物，根据本机 Herdr agent 的实时状态切换动画和提示信息。

## 运行

```bash
npm install
npm start
```

运行需要 Node.js、Rust 工具链以及当前平台的 Tauri 2 系统依赖。

程序默认连接当前 Herdr session。可通过 Herdr 已有的环境变量选择其他实例：

```bash
HERDR_SESSION=work npm start
HERDR_SOCKET_PATH=/path/to/herdr.sock npm start
```

## 交互

- 左键拖拽宠物：通过 Tauri 调用系统原生窗口拖拽。
- 悬停宠物：显示 agent、状态、标题和工作目录气泡。
- 右键宠物：打开皮肤菜单或退出程序。

目前只有“经典像素”皮肤，菜单和状态结构已经支持继续增加皮肤。

## 窗口行为

- 窗口透明、无边框、不可缩放，并保持在普通窗口之上。
- Tauri 在创建窗口时启用置顶和所有工作区可见。
- macOS 在窗口首次显示前使用 `Accessory` 激活策略，并原生设置 `NSScreenSaverWindowLevel`、`CanJoinAllSpaces` 和 `FullScreenAuxiliary`。
- macOS 锁屏、安全桌面和 DRM 保护画面不允许第三方窗口覆盖。

## 性能设计

- Herdr socket reconcile 和事件订阅运行在 Rust 后台线程，WebView 不执行 socket 或文件 I/O。
- 拖拽由系统窗口管理器完成，不通过逐帧 IPC 传输坐标。
- 常规动画每 400ms 更新一次；鼠标和菜单输入仍由窗口事件即时触发。

产品需求、验收标准和后续功能见 [docs/requirements.md](docs/requirements.md)。
