# Phase 3: Menu Bar App Design

Minimal system tray application for controlling the voice-controllm daemon.

## Decisions

- **No Tauri** - Too heavy for a tray-only app with no window
- **`tray-icon` + `muda` + `tao`** - Lightweight, cross-platform Rust crates from the Tauri ecosystem
- **App manages daemon** - Spawns daemon on launch, shuts it down on quit
- **CLI still works independently** - Both CLI and menu bar are control surfaces for the shared daemon
- **Icons from Lucide** - MIT-licensed, clean line icons. Monochrome mic template with colored state dots

## Crate Structure

```
menubar/
├── Cargo.toml          # tao, tray-icon, muda, tokio, proto dependency
├── src/
│   ├── main.rs         # Entry point: start daemon, event loops
│   ├── tray.rs         # Tray icon + menu construction, state-driven updates
│   ├── client.rs       # gRPC client (same pattern as CLI)
│   └── icons.rs        # Icon loading + state variants
└── icons/
    ├── mic-template.png       # 22x22 monochrome mic
    ├── mic-template@2x.png    # 44x44 Retina
    ├── mic-listening.png      # mic + green dot
    ├── mic-listening@2x.png
    ├── mic-paused.png         # mic + gray dot
    ├── mic-paused@2x.png
    ├── mic-init.png           # mic + yellow dot
    ├── mic-init@2x.png
    ├── mic-error.png          # mic + red dot
    └── mic-error@2x.png
```

## Threading Model

Two event loops run concurrently:

- **GUI thread (main)** - `tao` event loop drives the tray icon and menu. Must be the main thread on macOS.
- **Async runtime (background)** - Tokio runtime on a separate thread handles gRPC communication.

```
Main Thread (tao)              Background Thread (tokio)
─────────────────              ────────────────────────
EventLoop::run()               tokio::runtime::Runtime
  ├─ menu click ──────────────▶ command_rx → gRPC call
  │                              (StartListening, StopListening, etc.)
  │
  ◀─────────────────────────── state_tx ← Subscribe() stream
  tray icon update                (StateChange, InitProgress, etc.)
  menu rebuild
```

**Channels:**
- `std::sync::mpsc::Sender<Command>` - menu clicks → async runtime
- `tao::event_loop::EventLoopProxy<AppEvent>` - async runtime → GUI thread (wakes tao event loop)

**Startup sequence:**
1. `main()` creates the `tao` event loop (must happen first on macOS main thread)
2. Spawn a std thread running tokio runtime for all async work
3. Async thread spawns daemon if needed, connects, subscribes to events
4. State updates flow back to the GUI thread via `EventLoopProxy`
5. `EventLoop::run()` takes over the main thread

## Daemon Lifecycle

**On launch:**
1. Check if daemon is already running (probe Unix socket with `GetStatus`)
2. If not running, spawn daemon as detached child process (same as `vcm start`)
3. Poll for connection readiness (socket exists + `GetStatus` succeeds)
4. Call `Subscribe()` to get the event stream

**Connection loss:**
If the event stream drops or a gRPC call fails, switch to error state. Show "Disconnected" in menu. Attempt reconnect every 2 seconds.

**On quit:**
1. Send `Shutdown()` to daemon
2. Wait briefly for acknowledgment (short timeout)
3. Exit the process

## Menu Structure

Menu updates dynamically based on daemon state.

**Initializing:**
```
┌──────────────────────────┐
│ ◌ Initializing...  45%   │  ← disabled status text
├──────────────────────────┤
│   Quit                   │
└──────────────────────────┘
```

**Paused:**
```
┌──────────────────────────┐
│ ○ Paused                 │  ← disabled status text
├──────────────────────────┤
│   Start Listening        │
│ ────────────────────── │
│   Quit                   │
└──────────────────────────┘
```

**Listening:**
```
┌──────────────────────────┐
│ ● Listening              │  ← disabled status text
├──────────────────────────┤
│   Pause Listening        │
│ ────────────────────── │
│   Quit                   │
└──────────────────────────┘
```

**Error / Disconnected:**
```
┌──────────────────────────┐
│ ✕ Disconnected           │  ← disabled status text
├──────────────────────────┤
│   Quit                   │
└──────────────────────────┘
```

## Tray Icon States

Monochrome mic template image (Lucide `mic` icon) with colored dot overlay:

| State | Dot Color | Description |
|-------|-----------|-------------|
| Listening | Green | Actively capturing and transcribing |
| Paused | Gray | Ready but not listening |
| Initializing | Yellow | Downloading/loading models |
| Error/Disconnected | Red | Daemon error or connection lost |

## gRPC Integration

Reuses the `proto` crate and the same Unix socket connection pattern as the CLI (`cli/src/client.rs`).

| Menu Item | gRPC Call |
|-----------|-----------|
| Start Listening | `StartListening()` |
| Pause Listening | `StopListening()` |
| Quit | `Shutdown()` then exit |
