use byteorder::{ByteOrder, LittleEndian};

/// Custom hash function for Equix
pub fn custom_hash(input: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let mut state: u64 = 0x1234567890abcdef;

    for &byte in input.iter() {
        state = state.wrapping_mul(0x1bd11bdaa9fc1a22).wrapping_add(byte as u64);
    }

    LittleEndian::write_u64(&mut hash[0..8], state);
    hash
}

/// Custom difficulty calculation based on the new hash function
pub fn calculate_difficulty(hash: &[u8; 32]) -> u32 {
    let leading_zeroes = hash.iter().take_while(|&&byte| byte == 0).count();
    leading_zeroes as u32
}
