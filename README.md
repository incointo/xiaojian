# TickNest

<p align="center">
	<b>A tray-first desktop to-do app that stays out of your way.</b><br />
	Built with Tauri 2, Rust, and Vanilla HTML/CSS/JS.
</p>

<p align="center">
	<img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-blue" />
	<img alt="Rust" src="https://img.shields.io/badge/Rust-stable-orange" />
	<img alt="Platform" src="https://img.shields.io/badge/Platform-Windows-lightgrey" />
	<img alt="UI" src="https://img.shields.io/badge/UI-Compact-success" />
</p>

TickNest is a lightweight productivity companion for quick task capture and completion. It opens fast, lives in the tray, and keeps your list persisted locally with zero account setup.

## Why TickNest

- Minimal, distraction-free floating window
- Fast task flow: add, reorder, complete, done
- Local-first persistence with a simple tasks.json file
- No cloud dependency, no login, no noise

## Features

- Drag-and-drop reordering with a dedicated handle
- Completion animation before task removal
- Empty-state interface when list is blank
- Local JSON persistence via Rust commands
- Single-instance app behavior
- Tray integration:
	- Left click shows and focuses the window
	- Right click opens tray menu with Exit
- Startup placement at bottom-right of the primary monitor

## Preview

Add your screenshots or GIFs and keep this section updated as UI evolves.

![TickNest main view](docs/images/ticknest-main.png)
![TickNest drag and reorder](docs/images/ticknest-drag.gif)

## Tech Stack

- Tauri 2
- Rust
- Vanilla HTML, CSS, JavaScript

## Project Structure

- src: frontend assets and UI
- src-tauri: Rust backend, tray/window lifecycle, Tauri config

## Getting Started

### Prerequisites

- Node.js 18+
- Rust (stable)
- Tauri OS prerequisites: https://tauri.app/start/prerequisites/

### Install

```bash
npm install
```

### Development

```bash
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

## App Behavior

### Single Instance

If TickNest is already running, launching it again will focus and reveal the existing window instead of creating a second instance.

### Tray Menu

- Left-click tray icon: reveal and focus app
- Right-click tray icon: open menu
- Exit menu item: quit app cleanly

### Task Persistence

Tasks are saved to tasks.json next to the executable.

- Missing file is auto-created
- Reads and writes happen through Tauri commands in Rust

## Configuration

Main desktop configuration lives in src-tauri/tauri.conf.json.

Key settings include:

- Product name and app identifier
- Frameless always-on-top compact window
- App icon and bundle target settings

## Roadmap

- Add keyboard shortcuts for add and complete actions
- Add optional startup-on-login toggle
- Add task categories or quick tags
- Add import/export for tasks
- Add optional themes

## Contributing

Contributions are welcome.

1. Fork the repository
2. Create a feature branch
3. Commit your changes with clear messages
4. Open a pull request with screenshots for UI changes

## License

Choose a license and add it here (MIT is a common choice for this kind of app).
