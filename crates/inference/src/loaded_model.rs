use std::ffi::{CString, c_void};
use std::ptr::{self, NonNull};
use std::time::Instant;

use domovoi_inference_proto::{ErrorCode, GenerateRequest, GenerationStats, StopReason};
use domovoi_llama_sys::{
    llama_batch_get_one, llama_context, llama_decode, llama_free, llama_get_memory, llama_memory_clear, llama_model,
    llama_model_free, llama_model_get_vocab, llama_n_ctx, llama_sampler, llama_sampler_chain_add,
    llama_sampler_chain_default_params, llama_sampler_chain_init, llama_sampler_free, llama_sampler_init_dist,
    llama_sampler_init_grammar, llama_sampler_init_greedy, llama_sampler_init_temp, llama_sampler_init_top_k,
    llama_sampler_init_top_p, llama_sampler_sample, llama_set_abort_callback, llama_token, llama_token_to_piece,
    llama_tokenize, llama_vocab, llama_vocab_is_eog,
};

use crate::backend_error::BackendError;
use crate::cancel_flag::CancelFlag;
use crate::llama_backend::BATCH_SIZE;
use crate::utf8_assembler::Utf8Assembler;

const DECODE_ABORTED: i32 = 2;
const PIECE_CAPACITY: usize = 64;

#[derive(Debug)]
pub(crate) struct LoadedModel {
    model: NonNull<llama_model>,
    context: NonNull<llama_context>,
}

unsafe impl Send for LoadedModel {}

struct SamplerChain(NonNull<llama_sampler>);

impl Drop for SamplerChain {
    fn drop(&mut self) {
        unsafe { llama_sampler_free(self.0.as_ptr()) };
    }
}

enum Decoded {
    Complete,
    Aborted,
}

impl LoadedModel {
    pub(crate) unsafe fn new(model: NonNull<llama_model>, context: NonNull<llama_context>) -> Self {
        Self { model, context }
    }

    pub(crate) fn generate(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        unsafe {
            llama_set_abort_callback(self.context.as_ptr(), Some(abort_requested), cancel.as_callback_data());
        }
        let outcome = self.generate_with_abort(request, cancel, on_token);
        unsafe { llama_set_abort_callback(self.context.as_ptr(), None, ptr::null_mut()) };
        outcome
    }

    fn generate_with_abort(
        &mut self,
        request: &GenerateRequest,
        cancel: &CancelFlag,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationStats, BackendError> {
        unsafe { llama_memory_clear(llama_get_memory(self.context.as_ptr()), true) };
        let mut prompt = self.tokenize(&request.prompt)?;
        let capacity = unsafe { llama_n_ctx(self.context.as_ptr()) } as usize;
        if prompt.len() + request.max_tokens as usize > capacity {
            return Err(BackendError::new(
                ErrorCode::Internal,
                format!(
                    "{} prompt tokens and {} new tokens exceed the context of {capacity}",
                    prompt.len(),
                    request.max_tokens
                ),
            ));
        }
        let sampler = self.sampler(request)?;
        let started = Instant::now();
        let mut stats = GenerationStats {
            prompt_tokens: u32::try_from(prompt.len()).expect("the prompt fits the context"),
            generated_tokens: 0,
            prompt_duration: started.elapsed(),
            generation_duration: started.elapsed(),
            stop: StopReason::TokenLimit,
        };
        for chunk in prompt.chunks_mut(BATCH_SIZE as usize) {
            if let Decoded::Aborted = self.decode(chunk)? {
                stats.stop = StopReason::Cancelled;
                stats.prompt_duration = started.elapsed();
                return Ok(stats);
            }
        }
        stats.prompt_duration = started.elapsed();
        let generation_started = Instant::now();
        let vocab = self.vocab();
        let mut assembler = Utf8Assembler::default();
        while stats.generated_tokens < request.max_tokens {
            if cancel.is_raised() {
                stats.stop = StopReason::Cancelled;
                break;
            }
            let mut token = unsafe { llama_sampler_sample(sampler.0.as_ptr(), self.context.as_ptr(), -1) };
            if unsafe { llama_vocab_is_eog(vocab, token) } {
                stats.stop = StopReason::EndOfGeneration;
                break;
            }
            stats.generated_tokens += 1;
            let text = assembler.push(&piece(vocab, token));
            if !text.is_empty() {
                on_token(&text);
            }
            if let Decoded::Aborted = self.decode(std::slice::from_mut(&mut token))? {
                stats.stop = StopReason::Cancelled;
                break;
            }
        }
        let tail = assembler.finish();
        if !tail.is_empty() {
            on_token(&tail);
        }
        stats.generation_duration = generation_started.elapsed();
        Ok(stats)
    }

    fn vocab(&self) -> *const llama_vocab {
        unsafe { llama_model_get_vocab(self.model.as_ptr()) }
    }

    pub(crate) fn count_tokens(&self, text: &str) -> Result<u32, BackendError> {
        let tokens = self.tokenize(text)?;
        Ok(u32::try_from(tokens.len()).expect("a token count fits in u32"))
    }

    fn tokenize(&self, text: &str) -> Result<Vec<llama_token>, BackendError> {
        let length =
            i32::try_from(text.len()).map_err(|_| BackendError::new(ErrorCode::Internal, "the prompt is too long"))?;
        let mut tokens = vec![0; text.len() + 2];
        let mut count = unsafe {
            llama_tokenize(
                self.vocab(),
                text.as_ptr().cast(),
                length,
                tokens.as_mut_ptr(),
                i32::try_from(tokens.len()).expect("the token buffer fits in i32"),
                true,
                true,
            )
        };
        if count < 0 {
            tokens.resize(count.unsigned_abs() as usize, 0);
            count = unsafe {
                llama_tokenize(
                    self.vocab(),
                    text.as_ptr().cast(),
                    length,
                    tokens.as_mut_ptr(),
                    i32::try_from(tokens.len()).expect("the token buffer fits in i32"),
                    true,
                    true,
                )
            };
        }
        let count = usize::try_from(count)
            .map_err(|_| BackendError::new(ErrorCode::Internal, "the prompt could not be tokenized"))?;
        tokens.truncate(count);
        Ok(tokens)
    }

    fn sampler(&self, request: &GenerateRequest) -> Result<SamplerChain, BackendError> {
        let chain = SamplerChain(
            NonNull::new(unsafe { llama_sampler_chain_init(llama_sampler_chain_default_params()) })
                .expect("llama.cpp allocates a sampler chain"),
        );
        if let Some(grammar) = &request.grammar {
            let grammar = CString::new(grammar.as_str())
                .map_err(|_| BackendError::new(ErrorCode::InvalidGrammar, "the grammar contains NUL"))?;
            let sampler = unsafe { llama_sampler_init_grammar(self.vocab(), grammar.as_ptr(), c"root".as_ptr()) };
            if sampler.is_null() {
                return Err(BackendError::new(
                    ErrorCode::InvalidGrammar,
                    "llama.cpp rejected the grammar",
                ));
            }
            unsafe { llama_sampler_chain_add(chain.0.as_ptr(), sampler) };
        }
        let sampling = request.sampling;
        unsafe {
            if sampling.temperature <= 0.0 {
                llama_sampler_chain_add(chain.0.as_ptr(), llama_sampler_init_greedy());
            } else {
                llama_sampler_chain_add(
                    chain.0.as_ptr(),
                    llama_sampler_init_top_k(i32::try_from(sampling.top_k).unwrap_or(i32::MAX)),
                );
                llama_sampler_chain_add(chain.0.as_ptr(), llama_sampler_init_top_p(sampling.top_p as f32, 1));
                llama_sampler_chain_add(chain.0.as_ptr(), llama_sampler_init_temp(sampling.temperature as f32));
                llama_sampler_chain_add(chain.0.as_ptr(), llama_sampler_init_dist(sampling.seed));
            }
        }
        Ok(chain)
    }

    fn decode(&mut self, tokens: &mut [llama_token]) -> Result<Decoded, BackendError> {
        let count = i32::try_from(tokens.len()).expect("a batch fits in i32");
        match unsafe { llama_decode(self.context.as_ptr(), llama_batch_get_one(tokens.as_mut_ptr(), count)) } {
            0 => Ok(Decoded::Complete),
            DECODE_ABORTED => Ok(Decoded::Aborted),
            status => Err(BackendError::new(
                ErrorCode::Internal,
                format!("llama_decode failed with {status}"),
            )),
        }
    }
}

impl Drop for LoadedModel {
    fn drop(&mut self) {
        unsafe {
            llama_free(self.context.as_ptr());
            llama_model_free(self.model.as_ptr());
        }
    }
}

fn piece(vocab: *const llama_vocab, token: llama_token) -> Vec<u8> {
    let mut buffer = vec![0_u8; PIECE_CAPACITY];
    loop {
        let length = unsafe {
            llama_token_to_piece(
                vocab,
                token,
                buffer.as_mut_ptr().cast(),
                i32::try_from(buffer.len()).expect("the piece buffer fits in i32"),
                0,
                false,
            )
        };
        match usize::try_from(length) {
            Ok(length) => {
                buffer.truncate(length);
                return buffer;
            }
            Err(_) => buffer.resize(length.unsigned_abs() as usize, 0),
        }
    }
}

unsafe extern "C" fn abort_requested(data: *mut c_void) -> bool {
    unsafe { CancelFlag::is_raised_at(data) }
}
