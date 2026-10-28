# 小笺

小笺是一个面向 Windows 10/11 的轻量待办便笺，使用 Tauri 2、Rust、SQLite 和原生 HTML/CSS/JavaScript。界面保持为单个小窗口，不依赖前端框架；任务只保存在本机。

项目从 [TickNest](https://github.com/AakashVarma1995/TickNest) 派生，保留上游 MIT 许可和版权信息，详见 [LICENSE](LICENSE)。

## 功能

- 记录今日任务和每日重复任务；任务可以完成、撤销、改名或删除。
- 删除前会在窗口内确认，长标题自动换行，确认框适配最小窗口；默认选中取消，支持 Esc 及点击遮罩取消。
- 今日任务按 Windows 本地日期创建。未完成事项跨日后顺延；每日任务在新的一天自动恢复未完成。
- 已完成的今日任务只在完成当天显示，第二天自动从主窗口和托盘清单隐藏；历史记录保留在本机数据库。应用持续运行时跨日刷新最多延迟约一分钟，重新打开窗口或重启应用也会刷新。
- 退出时等待正在进行的任务保存；保存失败时窗口会保持打开。
- 主窗口高度随任务内容、文字换行和已完成列表的展开自动调整；内容过多时列表滚动，输入栏保持可见。宽度由用户手动调整，自动适配不会更改宽度。
- 默认宽度 360、最小宽度 340、最小高度 300，均为逻辑像素；高度上限 720，屏幕工作区不足时按实际空间缩小。随窗口所在屏幕的缩放比例换算，移动到另一显示器时重新适配。
- 窗口贴合边缘，始终置顶，按住顶部标题栏可自由拖动。点关闭按钮会收进系统托盘，应用继续运行。
- 托盘左键打开便笺，右键打开浮动待办清单。清单按“今日、每日”分组，勾选后保留已完成项，也可取消勾选；与主窗口实时同步，连续勾选不会关闭。点击外部或按 Esc 收起，底部可打开便笺或退出。
- 标题栏的清单按钮也能打开同一浮动清单。任务文字为 16px，辅助文字同步放大。
- 快捷清单初始化失败时会丢弃失效窗口，后续打开重新创建；同时提供带勾选状态的系统备用菜单，可操作任务、打开主窗口及退出。
- 标题按纯文本显示。所有数据保存在本机 SQLite 数据库，不连接云服务。
- 0.1.6 移除随手记界面及其读写入口；旧版随手记数据仍保留在原数据库中，不清空历史内容。

## 免安装使用

可以直接双击免安装版 `小笺.exe`，无需安装小笺，也不需要 Node.js、Rust 或开发工具。发布版的界面和图标已包含在可执行文件内；构建后对应的原始文件是 `src-tauri/target/release/xiaojian.exe`。

免安装版仍依赖 Microsoft Edge WebView2 Runtime。当前开发电脑已具备此运行时；其他 Windows 10/11 电脑如果缺少它，需要先安装 WebView2 Runtime。数据保存在下面的当前用户目录中，不会随 exe 一起移动。关闭窗口会收进系统托盘；要完全退出，请在托盘右键浮动清单底部选择退出。

## 数据位置

数据库文件位于当前 Windows 用户的应用数据目录：

```text
%APPDATA%\com.xiaojian.todo\xiaojian.sqlite3
```

数据库损坏或版本较新时，小笺会显示错误，不会自动删除或重置数据库文件。需要备份或迁移数据时，请先退出应用，再复制整个 `com.xiaojian.todo` 目录。

清单打开失败时会显示提示；窗口打开及焦点诊断记录位于 `%LOCALAPPDATA%\com.xiaojian.todo\logs\window-events.log`，文件达到 256 KiB 后自动重置，不记录任务标题或便笺内容。0.1.4 已修复主窗口和浮动清单的 WebView2 启动参数不一致导致清单无法打开的问题。

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

只构建免安装 exe（输出为 `src-tauri/target/release/xiaojian.exe`，可直接改名为 `小笺.exe`）：

```powershell
npm run tauri build -- --no-bundle
```

构建 Windows 安装包：

```powershell
npm run tauri build
```

Tauri 窗口、应用标识和 Windows 安装包配置位于 `src-tauri/tauri.conf.json`；Rust 命令、SQLite 存储和托盘逻辑位于 `src-tauri/src/`。
