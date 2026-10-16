use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use domovoi_llama_sys::{
    llama_backend_init, llama_model, llama_model_default_params, llama_model_free, llama_model_get_vocab,
    llama_model_load_from_file, llama_sampler, llama_sampler_accept, llama_sampler_apply, llama_sampler_free,
    llama_sampler_init_grammar, llama_token, llama_token_data, llama_token_data_array, llama_token_to_piece,
    llama_vocab, llama_vocab_eos, llama_vocab_n_tokens,
};

const MINIMAL_JSON_GRAMMAR: &CStr = cr#"root ::= "{" space (pair ("," space pair){0,3})? "}" space
pair ::= "\"" [a-z]{1,8} "\"" space ":" space [0-9]{1,4} space
space ::= " "?"#;
const BROKEN_GRAMMAR: &CStr = cr#"root ::= ( "{" "#;
const ROOT_RULE: &CStr = c"root";
const PIECE_CAPACITY: usize = 64;
const TINY_RANDOM: &str = "assets/test/tiny-random.gguf";
const STORIES: &str = "target/test-models/stories260K.gguf";
const TINY_RANDOM_TOKENS: i32 = 259;
const STORIES_TOKENS: i32 = 512;

struct Model(*mut llama_model);

impl Drop for Model {
    fn drop(&mut self) {
        unsafe { llama_model_free(self.0) };
    }
}

struct Sampler(*mut llama_sampler);

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe { llama_sampler_free(self.0) };
    }
}

fn workspace_file(relative: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative);
    assert!(
        path.is_file(),
        "{} is missing; run cargo xtask fetch-test-models",
        path.display()
    );
    path
}

fn vocabulary_only(path: &Path) -> Model {
    let path = CString::new(path.as_os_str().as_bytes()).expect("the model path has no NUL");
    let mut params = unsafe { llama_model_default_params() };
    params.vocab_only = true;
    let model = unsafe { llama_model_load_from_file(path.as_ptr(), params) };
    assert!(!model.is_null(), "{path:?} loads as a vocabulary");
    Model(model)
}

fn piece(vocab: *const llama_vocab, token: llama_token) -> Vec<u8> {
    let mut buffer = [0_u8; PIECE_CAPACITY];
    let length = unsafe {
        llama_token_to_piece(
            vocab,
            token,
            buffer.as_mut_ptr().cast(),
            i32::try_from(buffer.len()).expect("the buffer length fits in i32"),
            0,
            true,
        )
    };
    let length = usize::try_from(length).expect("every test token piece fits the buffer");
    buffer[..length].to_vec()
}

fn token_with_piece(vocab: *const llama_vocab, text: &[u8]) -> llama_token {
    let count = unsafe { llama_vocab_n_tokens(vocab) };
    (0..count)
        .find(|token| piece(vocab, *token) == text)
        .unwrap_or_else(|| panic!("the vocabulary has a token for {:?}", String::from_utf8_lossy(text)))
}

fn grammar_logits(sampler: &Sampler, vocab: *const llama_vocab) -> Vec<f32> {
    let count = unsafe { llama_vocab_n_tokens(vocab) };
    let mut candidates: Vec<llama_token_data> = (0..count)
        .map(|id| llama_token_data { id, logit: 0.0, p: 0.0 })
        .collect();
    let mut array = llama_token_data_array {
        data: candidates.as_mut_ptr(),
        size: candidates.len(),
        selected: -1,
        sorted: false,
    };
    unsafe { llama_sampler_apply(sampler.0, &raw mut array) };
    candidates.iter().map(|candidate| candidate.logit).collect()
}

fn index(token: llama_token) -> usize {
    usize::try_from(token).expect("token ids are non-negative")
}

fn allowed(logits: &[f32], token: llama_token) -> bool {
    logits[index(token)].is_finite()
}

fn rejected(logits: &[f32], token: llama_token) -> bool {
    logits[index(token)].is_infinite() && logits[index(token)].is_sign_negative()
}

fn check_grammar_sampling(model_path: &Path, expected_tokens: i32) {
    let model = vocabulary_only(model_path);
    let vocab = unsafe { llama_model_get_vocab(model.0) };
    assert_eq!(unsafe { llama_vocab_n_tokens(vocab) }, expected_tokens);

    let broken = unsafe { llama_sampler_init_grammar(vocab, BROKEN_GRAMMAR.as_ptr(), ROOT_RULE.as_ptr()) };
    assert!(broken.is_null(), "a broken grammar yields no sampler");

    let sampler =
        Sampler(unsafe { llama_sampler_init_grammar(vocab, MINIMAL_JSON_GRAMMAR.as_ptr(), ROOT_RULE.as_ptr()) });
    assert!(!sampler.0.is_null(), "the minimal JSON grammar compiles");
    let open = token_with_piece(vocab, b"{");
    let close = token_with_piece(vocab, b"}");
    let end = unsafe { llama_vocab_eos(vocab) };

    let first = grammar_logits(&sampler, vocab);
    assert!(allowed(&first, open));
    assert!(rejected(&first, end), "end of generation needs an empty stack");
    let count = unsafe { llama_vocab_n_tokens(vocab) };
    for token in 0..count {
        if !piece(vocab, token).starts_with(b"{") {
            assert!(rejected(&first, token), "token {token} cannot open the object");
        }
    }

    unsafe { llama_sampler_accept(sampler.0, open) };
    let inside = grammar_logits(&sampler, vocab);
    assert!(allowed(&inside, close));
    assert!(rejected(&inside, end), "an open object cannot end the generation");

    unsafe { llama_sampler_accept(sampler.0, close) };
    let closed = grammar_logits(&sampler, vocab);
    assert!(allowed(&closed, end), "a closed object may end the generation");
    assert!(rejected(&closed, open));
}

#[test]
fn grammar_sampling_works_on_the_vocabularies_of_both_test_models() {
    unsafe { std::env::set_var("GGML_METAL_DEVICES", "0") };
    unsafe { llama_backend_init() };
    check_grammar_sampling(&workspace_file(TINY_RANDOM), TINY_RANDOM_TOKENS);
    check_grammar_sampling(&workspace_file(STORIES), STORIES_TOKENS);
}
