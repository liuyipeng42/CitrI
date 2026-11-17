<p align="center">
  <img src="src/assets/logo.png" width="96" alt="CitrI logo" />
</p>

<h1 align="center">CitrI</h1>

<p align="center">
  一款轻量级划词助手，让常用操作始终触手可及。
</p>

柑橘总让人联想到清新、轻快与醒目，这也正是 CitrI 希望带来的使用感受：随手唤出、快速处理文字，然后继续专注于眼前的工作。

**CitrI** 这个名字结合了 *citrus*（柑橘）与 **I 形文本光标**。一颗柑橘由许多独立的果瓣组成，就像 CitrI 中一个个可以自由组合的 Action：翻译、解释、搜索、改写、复制、调用 AI……每一瓣都能独立使用，又共同组成一套完整的划词工具。

只需在任意应用中选中文字，CitrI 就会把需要的操作带到光标旁，无需切换窗口，也不会打断当前工作流。

> **清新、简单，始终触手可及。**

CitrI 是一个基于 **Tauri 2、Rust 和 Vue 3** 的轻量级跨应用划词助手。选中文本后，它会在光标附近显示工具条，让复制、搜索和 AI 处理触手可及。

## 功能亮点

- 跨应用监听拖选与双击选词，工具条跟随选区显示且不抢占焦点
- 内置复制、搜索、翻译、总结和解释动作，并支持润色及自定义 Prompt
- 动作可启用、停用、排序，也可以添加自定义动作
- 支持 OpenAI Chat Completions 兼容接口、SSE 流式输出和请求中止
- 支持多套 API 配置，每套配置可独立保存 URL、Key、模型、系统 Prompt 和额外参数
- 结果窗口支持 Markdown、复制、重新生成、停止、置顶、透明度和自动关闭
- 本地保存 AI 请求原文、实际 Prompt 和处理结果，支持搜索、分页浏览、弹窗查看详情与清空
- 支持 ZIP 导入导出设置与历史记录，以及 WebDAV 自动多设备同步
- 工具条支持紧凑模式；界面支持字号、预设强调色和任意自定义颜色
- 支持默认过滤、黑名单和白名单，控制 CitrI 在哪些应用中响应
- 支持系统托盘、单实例运行和首次启动设置引导
- 配置以本地 JSON 原子写入；API Key 不会暴露给工具条或结果窗口

## 使用方式

1. 在任意受支持的应用中拖选文字，或双击选中单词。
2. 在选区旁出现的工具条中选择动作。
3. 复制动作会直接写入剪贴板，搜索动作会打开浏览器；AI 动作会打开独立结果窗口并流式显示内容。
4. 可在设置页调整动作顺序、API 配置、窗口行为、界面外观和应用过滤规则。

## 环境要求

- [Node.js](https://nodejs.org/) 20.19.x 或 22.12+
- [Rust](https://www.rust-lang.org/tools/install) 1.85+
- 对应平台的 [Tauri 2 前置依赖](https://v2.tauri.app/start/prerequisites/)

### Windows

划词通过 Windows UI Automation 直接读取文本选区，不会模拟 Ctrl+C 或临时修改剪贴板。应用若未公开可访问的文本选区，工具条可能不会出现；不会退回模拟复制。此方式避免向终端发送可能中断命令的 Ctrl+C。macOS 和 Linux 的取词方式不受此适配影响。

准备以下环境：

- Rust MSVC 工具链
- Visual Studio Build Tools，其中勾选“使用 C++ 的桌面开发”和 Windows SDK
- WebView2 Runtime（Windows 10/11 通常已安装）

安装 Rust 后执行：

```powershell
rustup default stable-msvc
rustup target add x86_64-pc-windows-msvc
```

### macOS

安装 Xcode Command Line Tools：

```bash
xcode-select --install
```

首次运行后，在“系统设置 → 隐私与安全性 → 辅助功能”中允许 CitrI，并重新启动应用。全局鼠标监听和模拟复制依赖此权限。

### Debian / Ubuntu Linux

安装 Tauri 和 X11 开发依赖：

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  libx11-dev libxtst-dev libxi-dev
```

Linux 的全局手势监听和选区读取目前基于 X11。原生 Wayland 支持受桌面环境限制，建议使用 X11 会话；XWayland 下能否正常工作取决于桌面环境和目标应用。

## 快速开始

安装依赖并启动开发版本：

```bash
npm install
npm run tauri dev
```

首次启动时，CitrI 会打开设置窗口：

1. 填写 API URL、API Key 和模型名称。
2. 点击“测试当前 API”确认连接可用。
3. 保存设置，然后在其他应用中选择一段文字进行测试。

API URL 可以填写基础地址，也可以填写完整端点：

```text
https://api.openai.com/v1
https://your-api.example/v1/chat/completions
```

填写基础地址时，CitrI 会自动补全 `/chat/completions`。

## 开发与构建

常用命令：

```bash
npm run tauri dev     # 启动桌面开发版本
npm run types:generate # 从 Rust 更新前端接口类型
npm run check         # TypeScript / Vue 类型检查
npm test              # 前端测试
npm run build         # 构建前端资源
npm run tauri build   # 构建桌面安装包
```

桌面安装包需要在目标操作系统上构建，输出位于 `src-tauri/target/release/bundle/`。

完整校验：

```bash
npm run check
npm test
npm run build
cd src-tauri
cargo fmt --all -- --check
cargo check --tests
```

涉及全局鼠标监听、辅助功能、系统托盘和真实 WebView 的行为，仍需在目标桌面系统上进行人工冒烟测试。

## 项目结构

```text
CitrI/
├── src/
│   ├── features/               按功能组织页面、样式、状态和测试
│   │   ├── action/             AI 结果窗口与思考流
│   │   ├── toolbar/            划词工具条
│   │   ├── settings/           设置窗口、编辑状态、API 与基础面板
│   │   ├── history/            历史查询与详情
│   │   └── sync/               备份与 WebDAV 设置
│   ├── components/             共享界面组件
│   ├── lib/                    IPC、事件清理、Markdown、主题和调度工具
│   └── generated/              从 Rust 自动生成的接口类型
├── src-tauri/src/
│   ├── assistant/              AI 请求、SSE 协议与取消处理
│   ├── commands/               按功能分类的 IPC 命令与窗口权限校验
│   ├── data/                   备份格式、事务、同步配置与 WebDAV
│   ├── desktop/                窗口生命周期、托盘和设置应用
│   ├── history/                SQLite 历史、分页、搜索与合并
│   ├── http.rs                 共享 HTTP 客户端连接池
│   ├── models/                 按领域分类的接口类型、默认值和输入清洗
│   ├── selection/              输入监听、选区读取、过滤和工具条定位
│   │   └── windows/            Windows hook 恢复与 UI Automation
│   ├── settings.rs             配置存储
│   ├── state.rs                应用共享状态
│   ├── storage.rs              共享原子文件写入
│   └── lib.rs                  应用初始化与命令注册
├── scripts/                    开发维护脚本
└── src-tauri/capabilities/      Tauri 2 窗口能力声明
```

## 安全与隐私

- 完整设置仅能由设置窗口读取；工具条和结果窗口只会收到运行所需的公开字段。
- API Key 由 Rust 后端读取并保存在本机，不会发送到工具条或结果窗口。
- AI 动作会将选中文本发送到你配置的 API 服务；搜索动作会将关键词交给配置的搜索服务，复制动作不会发起网络请求。
- Markdown 输出会在渲染前清洗；外部链接仅允许由 Rust 打开 HTTP/HTTPS 地址。
- 配置保存在 Tauri 应用数据目录，并通过原子写入降低配置文件损坏风险。
