const REPLACEMENT: char = '\u{FFFD}';

#[derive(Debug, Default)]
pub struct Utf8Assembler {
    pending: Vec<u8>,
}

impl Utf8Assembler {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut text = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(valid) => {
                    text.push_str(valid);
                    self.pending.clear();
                    return text;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    text.push_str(std::str::from_utf8(&self.pending[..valid]).expect("the prefix is valid UTF-8"));
                    let Some(invalid) = error.error_len() else {
                        self.pending.drain(..valid);
                        return text;
                    };
                    text.push(REPLACEMENT);
                    self.pending.drain(..valid + invalid);
                }
            }
        }
    }

    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            String::new()
        } else {
            self.pending.clear();
            REPLACEMENT.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Utf8Assembler;

    #[test]
    fn split_characters_wait_for_their_remaining_bytes() {
        let mut assembler = Utf8Assembler::default();
        let word = "Домовой".as_bytes();
        assert_eq!(assembler.push(&word[..3]), "Д");
        assert_eq!(assembler.push(&word[3..]), "омовой");
        assert_eq!(assembler.push(&[0xE2, 0x80]), "");
        assert_eq!(assembler.push(&[0xA6]), "…");
    }

    #[test]
    fn invalid_bytes_become_replacement_characters() {
        let mut assembler = Utf8Assembler::default();
        assert_eq!(assembler.push(&[b'a', 0xFF, b'b']), "a\u{FFFD}b");
        assert_eq!(assembler.push(&[0xD0]), "");
        assert_eq!(assembler.finish(), "\u{FFFD}");
    }
}
