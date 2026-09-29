# herdr-pet

[English](README.en.md) · 简体中文

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
- 双击宠物：聚焦当前展示的 Herdr agent，并激活承载 Herdr 的终端应用。
- 悬停宠物：显示当前 session 中全部 agent 的状态、标题和状态持续时间；点击 agent 行会聚焦对应 pane 并显示承载 Herdr 的终端窗口，点击行末消息图标可进入提示输入页。
- 点击气泡右上角设置按钮：切换语言、session、皮肤或退出程序。只有存在多个 session 时才显示 session 菜单。

气泡使用像素边框和尾巴，列表中的切换按钮与消息按钮分开。输入页支持多行中文、字符计数和草稿保留：Enter 换行，Ctrl/Cmd + Enter 发送；输入法组合输入时不会触发快捷发送。返回后再次打开同一 session 的同一 agent 可继续草稿，退出程序后草稿不保留。

语言选项位于设置菜单，不占用气泡标题栏；选择会保存在本机。首次启动按系统语言选择：中文系统使用中文，其他系统使用英文。发送页左上角的“<”返回列表，发送按钮位于输入框内。气泡会留出足够高度，至少完整展示一条 agent 信息或整个发送表单。

宠物自动跟随所选会话前台 Herdr 客户端的实际主题，包括自定义配色、明暗模式、气泡背景与文字、按钮、徽标和动画状态色。后台每 5 秒刷新。需要支持 `client.theme.get` 的新版 Herdr 服务端与客户端；旧版本、没有兼容客户端或终端未返回某个颜色时，使用宠物默认配色。此功能不会修改用户主题。

发送期间禁止重复提交或切换目标；等待 Herdr 返回后显示“已提交给 Herdr”，这表示提示已提交，不表示 agent 已完成任务。发送失败时显示错误并保留正文，可手动重试；不自动重试。

blocked agent 的数量显示在宠物徽标上。目前只有“经典像素”皮肤，菜单和状态结构已经支持继续增加皮肤。

自动显示终端窗口目前支持 macOS 上直接运行 Herdr 的 Ghostty。首次使用时，macOS 可能要求允许 Herdr 控制 Ghostty；拒绝后可在“系统设置 → 隐私与安全性 → 自动化”中重新授权。当前不支持在 `tmux` 或 GNU Screen 中定位具体 Ghostty 窗口。pane 聚焦成功而窗口激活失败时，宠物会显示明确错误。

## 窗口行为

- 窗口透明、无边框、不可缩放，并保持在普通窗口之上。
- 拖拽结束后会按照宠物所在显示器的完整边界调整透明窗口，宠物可以贴住屏幕底部，且宠物和气泡不会被屏幕边缘裁剪。
- 窗口位置会持久化；显示器布局改变后会自动移回可见区域。
- Tauri 在创建窗口时启用置顶和所有工作区可见。
- macOS 在窗口首次显示前使用 `Accessory` 激活策略，并原生设置 `NSScreenSaverWindowLevel`、`CanJoinAllSpaces` 和 `FullScreenAuxiliary`。
- macOS 锁屏、安全桌面和 DRM 保护画面不允许第三方窗口覆盖。

## 性能设计

- Herdr socket reconcile 和事件订阅运行在 Rust 后台线程，WebView 不执行 socket 或文件 I/O。
- 拖拽由系统窗口管理器完成，不通过逐帧 IPC 传输坐标。
- 常规动画每 400ms 更新一次；鼠标和菜单输入仍由窗口事件即时触发。
