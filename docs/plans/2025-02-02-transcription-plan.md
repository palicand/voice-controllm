# Transcription Implementation Plan

## Tasks

### 1. Model Manager (`models.rs`)

Create shared model download infrastructure.

- [ ] Define `ModelId` enum (SileroVad, Canary1b)
- [ ] Define `ModelInfo` struct (url, filename, size)
- [ ] Implement `ModelManager::new()` - uses config models dir
- [ ] Implement `ModelManager::ensure_model()` - check exists, download if missing
- [ ] Add download progress logging
- [ ] Unit tests for path resolution
- [ ] Integration test for download (can be ignored in CI initially)

**Dependencies:** None

### 2. Update VAD to use Model Manager

Refactor VAD to use auto-download instead of expecting model to exist.

- [ ] Update `VoiceActivityDetector::new()` to accept `ModelManager`
- [ ] Call `ensure_model(ModelId::SileroVad)` in constructor
- [ ] Update tests and examples
- [ ] Update CI to remove manual model download (optional, can keep as fallback)

**Dependencies:** Task 1

### 3. Transcriber Trait (`transcribe/mod.rs`)

Define the abstraction.

- [ ] Create `transcribe/` module directory
- [ ] Define `Transcriber` trait with `transcribe()` method
- [ ] Re-export from `lib.rs`

**Dependencies:** None

### 4. Canary Implementation (`transcribe/canary.rs`)

Implement Canary backend.

- [ ] Add Canary model info to `ModelId`
- [ ] Research Canary ONNX model input/output format
- [ ] Implement `CanaryTranscriber::new()` - loads ONNX model
- [ ] Implement `Transcriber::transcribe()` - run inference
- [ ] Handle resampling if input isn't 16kHz
- [ ] Unit tests with mock/simple cases
- [ ] Integration test with bundled audio file

**Dependencies:** Tasks 1, 3

### 5. Engine (`engine.rs`)

Coordinator that ties everything together.

- [ ] Define `Engine` struct with all components
- [ ] Implement `Engine::new()` - initializes capture, VAD, transcriber
- [ ] Implement `Engine::run()` - main loop with callback
- [ ] Add speech buffer management (start/append/clear)
- [ ] Handle VAD events properly
- [ ] Error handling - log and continue
- [ ] Unit tests with mock transcriber
- [ ] Integration test (may need to be manual/ignored)

**Dependencies:** Tasks 2, 4

### 6. Example Binary

Create runnable example for testing.

- [ ] Create `examples/transcribe_test.rs`
- [ ] Initialize engine with default config
- [ ] Print transcriptions to stdout
- [ ] Test manually with microphone

**Dependencies:** Task 5

## Order of Implementation

```
1. Model Manager ─────┬──▶ 2. Update VAD
                      │
3. Transcriber Trait ─┴──▶ 4. Canary ──▶ 5. Engine ──▶ 6. Example
```

Tasks 1 and 3 can be done in parallel. Task 4 needs both. Task 5 needs 2 and 4.

## Commit Strategy

One commit per task:
1. `feat(daemon): add model download manager`
2. `refactor(daemon): use model manager for VAD`
3. `feat(daemon): add transcriber trait`
4. `feat(daemon): add Canary transcription backend`
5. `feat(daemon): add transcription engine`
6. `feat(daemon): add transcription example`
