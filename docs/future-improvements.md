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

### CoreML Encoder Generation
**Status:** Manual process documented

On Apple Silicon, whisper.cpp can use CoreML for ~3x faster inference via the Apple Neural Engine. This requires a separate `.mlmodelc` encoder file alongside the GGML model.

**Current manual process:**
```bash
# Clone whisper.cpp
git clone --depth 1 https://github.com/ggml-org/whisper.cpp.git /tmp/whisper-cpp-coreml

# Generate CoreML encoder (requires Python 3.11, ~5-15 min)
nix-shell -p python311 --run "
  cd /tmp/whisper-cpp-coreml
  python3 -m venv .venv && source .venv/bin/activate
  pip install 'numpy<2' torch==2.1.0 coremltools openai-whisper ane_transformers
  cd models && ./generate-coreml-model.sh large-v3-turbo
  cp -r ggml-large-v3-turbo-encoder.mlmodelc ~/.local/share/voice-controllm/models/
"
```

**Requirements:**
- Python 3.11 (newer versions have compatibility issues with coremltools)
- Xcode command-line tools: `xcode-select --install`
- macOS Sonoma (14)+ recommended
- ~5GB disk space during generation

**First run:** The first transcription after installing the CoreML model is slow (~30s) while macOS compiles the model for the specific device. Subsequent runs are fast.

**Future automation:**
- Option 1: Host pre-compiled `.mlmodelc` files (model-specific, ~500MB each)
- Option 2: Add `vcm setup-coreml` command that runs this process
- Option 3: Detect missing CoreML encoder and prompt user to generate

**Note:** Only non-quantized models can be converted to CoreML. The quantized variants (e.g., `large-v3-turbo-q5_0`) are not compatible.

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
