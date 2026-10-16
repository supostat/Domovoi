use std::mem::{align_of, offset_of, size_of};

use domovoi_llama_sys::{llama_context_params, llama_model_params, llama_token_data, llama_token_data_array};

const MODEL_PARAMS_SIZE: usize = 80;
const MODEL_PARAMS_DEVICES: usize = 0;
const MODEL_PARAMS_LOAD_MODE: usize = 24;
const MODEL_PARAMS_VOCAB_ONLY: usize = 72;
const CONTEXT_PARAMS_SIZE: usize = 168;
const CONTEXT_PARAMS_ABORT_CALLBACK: usize = 120;
const CONTEXT_PARAMS_ABORT_CALLBACK_DATA: usize = 128;
const TOKEN_DATA_SIZE: usize = 12;
const TOKEN_DATA_ARRAY_SIZE: usize = 32;
const POINTER_ALIGNMENT: usize = 8;

#[test]
fn model_parameters_keep_the_layout_of_the_pinned_header() {
    assert_eq!(size_of::<llama_model_params>(), MODEL_PARAMS_SIZE);
    assert_eq!(align_of::<llama_model_params>(), POINTER_ALIGNMENT);
    assert_eq!(offset_of!(llama_model_params, devices), MODEL_PARAMS_DEVICES);
    assert_eq!(offset_of!(llama_model_params, load_mode), MODEL_PARAMS_LOAD_MODE);
    assert_eq!(offset_of!(llama_model_params, vocab_only), MODEL_PARAMS_VOCAB_ONLY);
}

#[test]
fn context_parameters_keep_the_layout_of_the_pinned_header() {
    assert_eq!(size_of::<llama_context_params>(), CONTEXT_PARAMS_SIZE);
    assert_eq!(align_of::<llama_context_params>(), POINTER_ALIGNMENT);
    assert_eq!(
        offset_of!(llama_context_params, abort_callback),
        CONTEXT_PARAMS_ABORT_CALLBACK
    );
    assert_eq!(
        offset_of!(llama_context_params, abort_callback_data),
        CONTEXT_PARAMS_ABORT_CALLBACK_DATA
    );
}

#[test]
fn sampler_candidates_keep_the_layout_of_the_pinned_header() {
    assert_eq!(size_of::<llama_token_data>(), TOKEN_DATA_SIZE);
    assert_eq!(size_of::<llama_token_data_array>(), TOKEN_DATA_ARRAY_SIZE);
}
