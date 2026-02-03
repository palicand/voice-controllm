# Future Improvements

Tracked enhancements for later implementation.

## Model Downloads

### Resumable Downloads
**Status:** Implemented with limitations

Resume download support has been implemented using HTTP Range headers. However, there is a known limitation:

**Limitation:** Hugging Face's CDN (cas-bridge.xethub.hf.co) uses signed URLs that don't support HTTP Range requests. When a download is interrupted and resumed:
1. The partial `.tmp` file is preserved
2. On restart, we request the remaining bytes via Range header
3. The CDN returns HTTP 416 (Range Not Satisfiable)
4. We fall back to deleting the partial file and restarting from scratch

**Workaround:** For large models (whisper-medium ~1.5GB, whisper-large-v3 ~3GB), ensure a stable network connection before starting the download.

**Future fix:** Consider alternative download sources that support Range requests, or implement chunked download with verification.

### Progress Bar Speed During Resume
**Status:** Known issue

When resuming a download, the progress bar shows inflated download speeds initially. This is because setting the initial position (existing bytes) is counted as "instant" progress by the indicatif library, skewing the speed calculation.

**Fix:** Reset the progress bar's elapsed time after setting the initial position, or use a custom speed calculation that only counts bytes received in the current session.

## Performance

### CoreML Encoder
**Status:** ✅ Implemented - auto-downloaded from Hugging Face

On Apple Silicon, whisper.cpp uses CoreML for ~3-8x faster inference via the Apple Neural Engine. The required `.mlmodelc` encoder files are now **automatically downloaded** from Hugging Face alongside the GGML models.

**How it works:**
1. When `ModelManager::ensure_model()` is called for a Whisper model
2. It downloads the GGML model (`.bin`) if missing
3. On macOS, it also downloads the pre-compiled CoreML encoder (`.mlmodelc.zip`)
4. Extracts the zip and removes it to save space

**First run:** The first transcription after downloading is slow (~30s) while macOS compiles the model for the specific hardware. This is cached by the system - subsequent runs are fast.

**Model sizes (approximate):**
| Model | GGML | CoreML Encoder |
|-------|------|----------------|
| tiny | 75MB | 50MB |
| base | 150MB | 90MB |
| small | 500MB | 250MB |
| medium | 1.5GB | 600MB |
| large-v3 | 3GB | 1.2GB |
| large-v3-turbo | 1.5GB | 1.1GB |

**Note:** Only non-quantized models have CoreML support. Quantized variants (e.g., `large-v3-turbo-q5_0`) fall back to CPU inference.

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

## Testing

### Integration Tests Require Models at Hardcoded Path
**Status:** Known issue

Integration tests in `daemon/tests/vad_integration.rs` look for models at a relative `models/` path within the project directory instead of using the standard `~/.local/share/voice-controllm/models/` location.

**Impact:** Tests fail in git worktrees or fresh clones until models are manually copied.

**Fix:** Update integration tests to use `ModelManager` or the standard config paths instead of hardcoded relative paths.
