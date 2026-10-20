# 小笺

小笺是一个面向 Windows 10/11 的轻量待办便笺，使用 Tauri 2、Rust、SQLite 和原生 HTML/CSS/JavaScript。界面保持为单个小窗口，不依赖前端框架；任务和便笺只保存在本机。

项目从 [TickNest](https://github.com/AakashVarma1995/TickNest) 派生，保留上游 MIT 许可和版权信息，详见 [LICENSE](LICENSE)。

## 功能

- 记录今日任务和每日重复任务；任务可以完成、撤销、改名或删除。
- 今日任务按 Windows 本地日期创建。未完成事项跨日后顺延；每日任务在新的一天自动恢复未完成。
- 便笺为纯文本，输入后延迟保存，离开输入框时立即保存。退出时会等待保存；保存失败时窗口会保持打开。
- 窗口始终置顶。点关闭按钮会收进系统托盘，应用继续运行；托盘左键打开窗口，右键菜单可以直接完成待办或退出。
- 标题和便笺按纯文本显示。所有数据保存在本机 SQLite 数据库，不连接云服务。

## 免安装使用

可以直接双击免安装版 `小笺.exe`，无需安装小笺，也不需要 Node.js、Rust 或开发工具。发布版的界面和图标已包含在可执行文件内；构建后对应的原始文件是 `src-tauri/target/release/xiaojian.exe`。

免安装版仍依赖 Microsoft Edge WebView2 Runtime。当前开发电脑已具备此运行时；其他 Windows 10/11 电脑如果缺少它，需要先安装 WebView2 Runtime。数据保存在下面的当前用户目录中，不会随 exe 一起移动。关闭窗口会收进系统托盘；要完全退出，请在托盘右键菜单中选择退出。

## 数据位置

数据库文件位于当前 Windows 用户的应用数据目录：

```text
%APPDATA%\com.xiaojian.todo\xiaojian.sqlite3
```

数据库损坏或版本较新时，小笺会显示错误，不会自动删除或重置数据库文件。需要备份或迁移数据时，请先退出应用，再复制整个 `com.xiaojian.todo` 目录。

## 开发环境

- Windows 10 或 Windows 11
- Node.js 18 或更新版本
- Rust stable 工具链（MSVC）
- Visual Studio C++ Build Tools 和 Windows SDK
- Microsoft Edge WebView2 Runtime

安装 JavaScript 工具并启动开发版：

```powershell
npm ci
npm run tauri dev
```

构建 Windows 安装包：

```powershell
npm run tauri build
```

Tauri 窗口、应用标识和 Windows 安装包配置位于 `src-tauri/tauri.conf.json`；Rust 命令、SQLite 存储和托盘逻辑位于 `src-tauri/src/`。
