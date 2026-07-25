# TickNest

A lightweight desktop to-do app built with Tauri 2, vanilla HTML/CSS/JS, and Rust.

TickNest is designed to stay out of your way: fast startup, compact UI, tray-first workflow, and simple local persistence.

## Highlights

- Compact floating to-do window
- Drag-and-drop reordering with a handle
- Completion animation before task removal
- Empty-state UI when there are no tasks
- Local JSON persistence for tasks
- System tray integration:
	- Left click shows and focuses the app
	- Right click opens a tray menu with Exit
- Single-instance behavior (second launch focuses the running app)
- Startup placement at bottom-right of the primary monitor

## Tech Stack

- Tauri 2
- Rust
- Vanilla HTML, CSS, JavaScript

## Project Structure

- src: Frontend files (HTML/CSS/JS assets)
- src-tauri: Rust backend, window/tray logic, Tauri config

## Requirements

- Node.js 18+
- Rust (stable)
- Tauri prerequisites for your OS:
	- https://tauri.app/start/prerequisites/

## Run Locally

1. Install dependencies

```bash
npm install
```

2. Start in development mode

```bash
npm run tauri dev
```

3. Build release binaries

```bash
npm run tauri build
```

## Data Storage

Tasks are saved to a file named tasks.json next to the application executable.

- If tasks.json does not exist, TickNest creates it automatically.
- Reads and writes are handled by Rust commands exposed to the frontend.

## Tray and Window Behavior

- The app runs as a single instance.
- Launching TickNest again does not create a second window; it focuses the running one.
- Window is positioned near the bottom-right of the primary monitor.

## Configuration

Core desktop settings are in src-tauri/tauri.conf.json, including:

- Product name and bundle identifier
- Window size and behavior (always-on-top, frameless, transparent)
- Bundle targets and app icons

## Development Notes

- Frontend distribution directory is configured as src.
- Main Rust entry and app wiring live in src-tauri/src/lib.rs.

## License

Add your preferred license here (for example MIT).
