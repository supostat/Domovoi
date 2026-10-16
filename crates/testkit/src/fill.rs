#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Byte(u8),
    Noise(u64),
}

impl Fill {
    pub(crate) fn chunk(self, offset: u64, length: usize) -> Vec<u8> {
        match self {
            Self::Byte(byte) => vec![byte; length],
            Self::Noise(seed) => (0..length as u64)
                .map(|index| noise_byte(seed, offset + index))
                .collect(),
        }
    }
}

fn noise_byte(seed: u64, position: u64) -> u8 {
    let mut state = seed ^ position.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    state ^= state >> 33;
    state = state.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    state ^= state >> 33;
    state.to_le_bytes()[0]
}
