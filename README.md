# 小笺

小笺是一个面向 Windows 10/11 的轻量桌面待办便笺，基于 Tauri 2、Rust 和原生 HTML/CSS/JavaScript。项目从 [TickNest](https://github.com/AakashVarma1995/TickNest) 派生，保留原项目的 MIT 许可和版权声明；具体许可见 [LICENSE](LICENSE)。

当前分支用于实现每日待办、今日待办、本地便笺和托盘快捷操作。界面使用 Tauri 静态前端目录，不使用 React/Vue 或前端打包框架。

## 开发环境

- Node.js 18 或更新版本
- Rust stable
- Windows 上的 Visual Studio C++ Build Tools 和 Windows SDK
- WebView2 Runtime

安装依赖并启动开发版：

```powershell
npm install
npm run tauri dev
```

构建 Windows 安装包：

```powershell
npm run tauri build
```

窗口和应用标识配置位于 `src-tauri/tauri.conf.json`。任务和便笺仅保存在本机应用数据目录；不会连接云服务。
