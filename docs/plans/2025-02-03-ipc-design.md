# Phase 2: IPC & CLI Design

gRPC-based communication between daemon and CLI.

## Scope

**In scope:**
- gRPC service definition (proto)
- Daemon gRPC server on Unix socket
- CLI gRPC client
- Commands: `start`, `stop`, `status`, `toggle`
- Event streaming (transcriptions + state changes)
- Daemon process management (fork/detach, PID file)

**Deferred:**
- Detailed stats (transcription count, audio levels, uptime)
- TCP transport (for debugging)
- `vcm test-mic`, `vcm transcribe <file>`
- launchd/systemd integration (Phase 4)
- Config commands via gRPC

## gRPC Service Definition

```protobuf
syntax = "proto3";
package voice_controllm;

service VoiceControllm {
  // Control
  rpc StartListening(Empty) returns (Empty);
  rpc StopListening(Empty) returns (Empty);
  rpc Shutdown(Empty) returns (Empty);

  // Query
  rpc GetStatus(Empty) returns (Status);

  // Streaming
  rpc Subscribe(Empty) returns (stream Event);
}

message Empty {}

message Status {
  oneof status {
    Healthy healthy = 1;
    Error error = 2;
  }
}

message Healthy {
  State state = 1;
}

enum State {
  STOPPED = 0;
  LISTENING = 1;
  PAUSED = 2;
}

message Error {
  string message = 1;
}

message Event {
  oneof event {
    StateChange state_change = 1;
    Transcription transcription = 2;
  }
}

message StateChange {
  oneof status {
    State new_state = 1;
    Error error = 2;
  }
}

message Transcription {
  string text = 1;
  double confidence = 2;
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                        Daemon                           │
│                                                         │
│  ┌──────────────┐    ┌──────────────┐                  │
│  │ gRPC Server  │───▶│  Controller  │                  │
│  │ (tonic)      │    │              │                  │
│  └──────────────┘    └──────┬───────┘                  │
│         ▲                   │                          │
│         │                   ▼                          │
│  ┌──────────────┐    ┌──────────────┐                  │
│  │  Subscribers │◀───│   Engine     │                  │
│  │  (broadcast) │    │ (Phase 1)    │                  │
│  └──────────────┘    └──────────────┘                  │
└─────────────────────────────────────────────────────────┘
         │
         │ Unix socket
         ▼
    ~/.local/state/voice-controllm/daemon.sock
```

**Components:**

- **gRPC Server**: tonic server on Unix socket, handles incoming RPCs
- **Controller**: Owns daemon state, translates RPCs to engine commands, broadcasts events
- **Engine**: Existing Phase 1 code (audio → VAD → transcribe → inject)
- **Subscribers**: tokio broadcast channel for `Subscribe` RPC receivers

## File Structure

**New crate:**
```
proto/
├── Cargo.toml
├── build.rs
└── src/
    ├── lib.rs
    └── voice_controllm.proto
```

**Daemon additions:**
- `daemon/src/server.rs` - gRPC server setup, service implementation
- `daemon/src/controller.rs` - State machine, event broadcasting

**State files:**
- Socket: `~/.local/state/voice-controllm/daemon.sock`
- PID file: `~/.local/state/voice-controllm/daemon.pid`

## CLI Commands

**`vcm start`:**
1. Check if daemon already running (PID file + process check)
2. If running, print "Daemon already running (PID: X)" and exit
3. Fork daemon as detached child
4. Daemon writes PID file, creates socket
5. CLI waits for socket (up to 2s)
6. Print "Daemon started (PID: X)"

**`vcm stop`:**
1. Connect via gRPC
2. Call `Shutdown` RPC
3. Daemon cleans up (stops engine, removes socket, removes PID file)
4. Print "Daemon stopped"

**`vcm status`:**
1. Check socket exists and daemon responds
2. Call `GetStatus` RPC
3. Print: "Listening", "Paused", "Error: {message}", or "Daemon not running"

**`vcm toggle`:**
1. Call `GetStatus` to check current state
2. If `LISTENING` → call `StopListening`, print "Paused"
3. If `PAUSED` → call `StartListening`, print "Listening"
4. If error or not running → print error

**Error handling:**
- Socket doesn't exist → "Daemon not running"
- Connection refused → "Daemon not running" (clean up stale socket)
- RPC fails → Print error message

## Dependencies

**Proto crate:**
- `tonic` - gRPC
- `prost` - protobuf

**Proto build:**
- `tonic-build`
- `prost-build`

**Daemon:**
- `tonic` - gRPC server
- `tokio-stream` - broadcast → gRPC stream adapter

**CLI:**
- `tonic` - gRPC client

## Implementation Order

1. **Proto crate** - service definition, codegen
2. **Daemon gRPC server (stub)** - tonic server, stub responses, Unix socket
3. **Controller & state machine** - real state transitions, broadcast channel
4. **Engine integration** - controller starts/stops engine, transcription events
5. **CLI client** - `status`, `toggle`, `stop` commands
6. **Daemon spawning** - `start` command, fork/detach, PID file

## Testing

- Unit tests for controller state machine
- Integration test: spawn daemon, connect, toggle, stop
- Manual test: full flow with actual transcription
