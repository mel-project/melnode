pub fn key_to_path(key: [u8; 32]) -> [bool; 256] {
    let mut toret = [false; 256];
    // enumerate each byte
    for (i, k_i) in key.iter().enumerate() {
        // walk through the bits
        for j in 0..8 {
            toret[i * 8 + j] = k_i & (0b1000_0000 >> j) != 0;
        }
    }
    toret
}
