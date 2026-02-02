# Speech Recognition Models Research

Evaluated for offline, local transcription on macOS (Apple Silicon).

## Summary

| Model | Status | Rust Support | Performance | Notes |
|-------|--------|--------------|-------------|-------|
| Whisper | ✅ Ready | whisper-rs (mature) | Good, CoreML 8-12x boost | Best choice now |
| Voxtral | 🔮 Future | None yet | SOTA, beats Whisper | Wait for GGML port |
| Canary | ❌ Skip | Complex ONNX | Good multilingual | Too complex for now |

## Whisper (OpenAI)

**Source:** [whisper.cpp](https://github.com/ggml-org/whisper.cpp), [whisper-rs](https://github.com/tazz4843/whisper-rs)

### Pros
- Mature C++ implementation with Rust bindings
- CoreML support for Apple Silicon (8-12x speedup)
- Simple API - handles preprocessing and decoding internally
- Wide model range: tiny (~75MB) to large-v3 (~3GB)
- Input format matches our pipeline: 16kHz mono f32

### Cons
- English-optimized models are better than multilingual for English
- Large models needed for best accuracy

### Models
- `tiny` / `tiny.en` - 75MB, fastest, lower accuracy
- `base` / `base.en` - 150MB, good balance
- `small` / `small.en` - 500MB, better accuracy
- `medium` / `medium.en` - 1.5GB, high accuracy
- `large-v3` - 3GB, best accuracy, multilingual only
- `large-v3-turbo` - 1.5GB, near large-v3 quality, 8x faster

### Integration
```rust
// whisper-rs handles everything
let ctx = WhisperContext::new_with_params(model_path, params)?;
let mut state = ctx.create_state()?;
state.full(params, &audio_samples)?;
let text = state.full_get_segment_text(0)?;
```

## Voxtral (Mistral AI)

**Source:** [Mistral announcement](https://mistral.ai/news/voxtral), [arXiv paper](https://arxiv.org/html/2507.13264v1)

### Pros
- State-of-the-art performance (beats Whisper large-v3, GPT-4o)
- Excellent European language support
- LLM-based: can do Q&A and summarization on audio
- Two sizes: 3B (edge) and 24B (production)
- Apache 2.0 license
- 32k context, handles 30-40 min audio

### Cons
- Brand new (July 2025)
- No Rust/C++ implementation yet
- Would require Python/transformers or wait for GGML port
- Larger models than Whisper equivalents

### Future Potential
When a GGML/whisper.cpp-style port appears, Voxtral could become the preferred backend due to superior accuracy and multilingual support.

## Canary (NVIDIA NeMo)

**Source:** [HuggingFace](https://huggingface.co/nvidia/canary-1b), [NVIDIA blog](https://developer.nvidia.com/blog/new-standard-for-speech-recognition-and-translation-from-the-nvidia-nemo-canary-model/)

### Pros
- Strong multilingual (EN, DE, FR, ES + translation)
- Good accuracy on benchmarks
- ONNX export available

### Cons
- Complex encoder-decoder architecture
- Requires separate mel spectrogram preprocessing
- Auto-regressive decoding loop needed
- Multiple ONNX files (encoder + decoder)
- No simple Rust integration path

### Why We Skipped
The complexity of implementing the full inference pipeline in Rust (mel spectrogram → encoder → decoder loop → token decoding) wasn't worth it when whisper-rs provides a simpler, working solution.

## Decision

**Use Whisper via whisper-rs** for initial implementation:
1. Mature, well-tested
2. Simple integration
3. CoreML acceleration on macOS
4. Good enough accuracy for dictation

**Keep Voxtral on radar** - add as backend when GGML port becomes available.

## References

- [whisper.cpp GitHub](https://github.com/ggml-org/whisper.cpp)
- [whisper-rs crate](https://crates.io/crates/whisper-rs)
- [WhisperKit (Swift/CoreML)](https://github.com/argmaxinc/WhisperKit)
- [Voxtral announcement](https://mistral.ai/news/voxtral)
- [Voxtral paper](https://arxiv.org/html/2507.13264v1)
- [Canary HuggingFace](https://huggingface.co/nvidia/canary-1b)
- [onnx-asr (Canary ONNX)](https://huggingface.co/istupakov/canary-1b-v2-onnx)
