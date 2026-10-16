"""Generate assets/test/tiny-random.gguf, a deterministic random llama model for CPU tests.

Architecture llama, one layer, n_embd 32, n_ff 64, two heads, F32 weights drawn from a normal distribution with
seed 0. The vocabulary is a byte-level SentencePiece one: <unk>, <s>, </s> and the 256 byte tokens <0x00>…<0xFF>,
so any text, JSON included, has a tokenization. Uses gguf-py from the pinned vendor/llama.cpp checkout.

Usage: python3 research/tiny_gguf.py
"""

import hashlib
import sys
from pathlib import Path

import numpy

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "vendor" / "llama.cpp" / "gguf-py"))

import gguf  # noqa: E402

OUTPUT = ROOT / "assets" / "test" / "tiny-random.gguf"
SEED = 0
WEIGHT_DEVIATION = 0.02
EMBEDDING = 32
FEED_FORWARD = 64
HEADS = 2
LAYERS = 1
CONTEXT = 512
RMS_EPSILON = 1e-5
ROPE_BASE = 10000.0
SPECIAL_TOKENS = [("<unk>", gguf.TokenType.UNKNOWN), ("<s>", gguf.TokenType.CONTROL), ("</s>", gguf.TokenType.CONTROL)]
UNKNOWN_ID = 0
BEGIN_ID = 1
END_ID = 2


def vocabulary():
    tokens = [text for text, _ in SPECIAL_TOKENS] + [f"<0x{byte:02X}>" for byte in range(256)]
    types = [kind for _, kind in SPECIAL_TOKENS] + [gguf.TokenType.BYTE] * 256
    return tokens, types


def main():
    random = numpy.random.default_rng(SEED)

    def weights(*shape):
        return random.normal(0.0, WEIGHT_DEVIATION, size=shape).astype(numpy.float32)

    def ones(size):
        return numpy.ones(size, dtype=numpy.float32)

    tokens, types = vocabulary()
    writer = gguf.GGUFWriter(OUTPUT, "llama")
    writer.add_context_length(CONTEXT)
    writer.add_embedding_length(EMBEDDING)
    writer.add_block_count(LAYERS)
    writer.add_feed_forward_length(FEED_FORWARD)
    writer.add_head_count(HEADS)
    writer.add_head_count_kv(HEADS)
    writer.add_rope_dimension_count(EMBEDDING // HEADS)
    writer.add_rope_freq_base(ROPE_BASE)
    writer.add_layer_norm_rms_eps(RMS_EPSILON)
    writer.add_file_type(gguf.LlamaFileType.ALL_F32)
    writer.add_tokenizer_model("llama")
    writer.add_token_list(tokens)
    writer.add_token_scores([0.0] * len(tokens))
    writer.add_token_types(types)
    writer.add_unk_token_id(UNKNOWN_ID)
    writer.add_bos_token_id(BEGIN_ID)
    writer.add_eos_token_id(END_ID)

    writer.add_tensor("token_embd.weight", weights(len(tokens), EMBEDDING))
    for layer in range(LAYERS):
        prefix = f"blk.{layer}"
        writer.add_tensor(f"{prefix}.attn_norm.weight", ones(EMBEDDING))
        for projection in ("attn_q", "attn_k", "attn_v", "attn_output"):
            writer.add_tensor(f"{prefix}.{projection}.weight", weights(EMBEDDING, EMBEDDING))
        writer.add_tensor(f"{prefix}.ffn_norm.weight", ones(EMBEDDING))
        writer.add_tensor(f"{prefix}.ffn_gate.weight", weights(FEED_FORWARD, EMBEDDING))
        writer.add_tensor(f"{prefix}.ffn_up.weight", weights(FEED_FORWARD, EMBEDDING))
        writer.add_tensor(f"{prefix}.ffn_down.weight", weights(EMBEDDING, FEED_FORWARD))
    writer.add_tensor("output_norm.weight", ones(EMBEDDING))
    writer.add_tensor("output.weight", weights(len(tokens), EMBEDDING))

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    writer.write_header_to_file()
    writer.write_kv_data_to_file()
    writer.write_tensors_to_file()
    writer.close()

    digest = hashlib.sha256(OUTPUT.read_bytes()).hexdigest()
    print(f"{OUTPUT.relative_to(ROOT)}: {OUTPUT.stat().st_size} bytes, sha256 {digest}")


if __name__ == "__main__":
    main()
