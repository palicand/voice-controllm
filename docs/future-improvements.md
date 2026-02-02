# Future Improvements

Tracked enhancements for later implementation.

## Model Downloads

### Resumable Downloads
**Priority:** Medium
**Context:** Large models (whisper-base ~150MB, large-v3 ~3GB) take time to download. Network interruptions require starting over.

**Solution:**
- Use HTTP Range headers to resume from last byte
- Keep `.tmp` file with progress metadata
- On restart, check existing `.tmp` size and request remaining bytes
- Validate final checksum after assembly

**Implementation notes:**
```rust
// Check for partial download
if temp_path.exists() {
    let existing_size = fs::metadata(&temp_path).await?.len();
    // Request remaining bytes with Range header
    let response = client
        .get(url)
        .header("Range", format!("bytes={}-", existing_size))
        .send()
        .await?;
    // Append to existing file...
}
```

## Transcription

### Streaming Transcription
**Priority:** High (after POC)
**Context:** Currently waits for SpeechEnd before transcribing. Users want real-time word-by-word output.

**Solution:**
- Whisper supports streaming via whisper.cpp's `whisper_full_with_state`
- Feed chunks while speaking, get partial results
- Update display as words are recognized

### Voxtral Backend
**Priority:** Low
**Context:** Mistral's Voxtral beats Whisper on accuracy. Currently no Rust/C++ port.

**Solution:**
- Monitor for GGML/whisper.cpp-style port
- Add as alternative backend when available
