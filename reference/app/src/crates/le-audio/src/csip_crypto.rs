//! The CSIS crypto toolbox (CSIS 1.0.1 section 4; v1 le_audio::csip in
//! le_audio_csip.{h,cpp}).
//!
//! Keys and values are 128-bit numbers in the spec's notation: byte 0 is
//! the most significant octet (the reverse of the over-the-air order). v1
//! ran [`self_test`] when the CSIP module started and logged a failure.
//!
//! AES-128 and AES-CMAC are implemented here (encryption only, a plain
//! S-box table, not constant time - as BTstack's rijndael.c that v1 used;
//! the crate has no dependencies) and checked against FIPS-197 and RFC 4493.

/// A 128-bit key or value, most significant octet first
pub type Key = [u8; 16];

/// AES-128 (the security function e)
pub fn aes128(key: &Key, input: &[u8; 16]) -> [u8; 16] {
    let rk = expand_key(key);
    let mut s = *input;
    add_round_key(&mut s, &rk[0]);
    for round_key in &rk[1..10] {
        sub_bytes(&mut s);
        shift_rows(&mut s);
        mix_columns(&mut s);
        add_round_key(&mut s, round_key);
    }
    sub_bytes(&mut s);
    shift_rows(&mut s);
    add_round_key(&mut s, &rk[10]);
    s
}

/// AES-CMAC (RFC 4493)
pub fn aes_cmac(key: &Key, msg: &[u8]) -> [u8; 16] {
    let l = aes128(key, &[0; 16]);
    let k1 = cmac_double(&l);
    let k2 = cmac_double(&k1);
    let blocks = msg.len().div_ceil(16).max(1);
    let complete = !msg.is_empty() && msg.len().is_multiple_of(16);
    let last_off = 16 * (blocks - 1);
    let mut last = [0u8; 16];
    if complete {
        for i in 0..16 {
            last[i] = msg[last_off + i] ^ k1[i];
        }
    } else {
        let rest = &msg[last_off..];
        last[..rest.len()].copy_from_slice(rest);
        last[rest.len()] = 0x80;
        for i in 0..16 {
            last[i] ^= k2[i];
        }
    }
    let mut x = [0u8; 16];
    for block in msg.chunks(16).take(blocks - 1) {
        for i in 0..16 {
            x[i] ^= block[i];
        }
        x = aes128(key, &x);
    }
    for i in 0..16 {
        x[i] ^= last[i];
    }
    aes128(key, &x)
}

/// The salt generation function s1 (4.3): AES-CMAC with a zero key
pub fn s1(m: &[u8]) -> Key {
    aes_cmac(&[0; 16], m)
}

/// The key derivation function k1 (4.4): AES-CMAC_T(P), T =
/// AES-CMAC_SALT(N)
pub fn k1(n: &[u8], salt: &Key, p: &[u8]) -> Key {
    let t = aes_cmac(salt, n);
    aes_cmac(&t, p)
}

/// SIRK encryption function sef (4.5): k1(K, s1("SIRKenc"), "csis") XOR
/// sirk, K = the LTK of the link
pub fn sef(k: &Key, sirk: &Key) -> Key {
    let key = k1(k, &s1(b"SIRKenc"), b"csis");
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = key[i] ^ sirk[i];
    }
    out
}

/// SIRK decryption function sdf (4.6): the same operation as [`sef`]
pub fn sdf(k: &Key, encrypted_sirk: &Key) -> Key {
    sef(k, encrypted_sirk)
}

/// RSI hash function sih (4.7): the 24-bit hash of `prand` (24 bits) under
/// `sirk`: e(k, r'), r' = padding || r (r in the least significant 24
/// bits), mod 2^24
pub fn sih(sirk: &Key, prand: u32) -> u32 {
    let mut r = [0u8; 16];
    r[13] = (prand >> 16) as u8;
    r[14] = (prand >> 8) as u8;
    r[15] = prand as u8;
    let e = aes128(sirk, &r);
    u32::from(e[13]) << 16 | u32::from(e[14]) << 8 | u32::from(e[15])
}

/// True if `rsi` (6 octets as advertised: hash, prand; least significant
/// octet first) was generated with `sirk`
pub fn rsi_matches(sirk: &Key, rsi: &[u8; 6]) -> bool {
    // RSI = prand || hash (prand most significant), sent least significant
    // octet first
    let hash = u32::from_le_bytes([rsi[0], rsi[1], rsi[2], 0]);
    let prand = u32::from_le_bytes([rsi[3], rsi[4], rsi[5], 0]);
    sih(sirk, prand) == hash
}

/// Checks the functions against the sample data of CSIS appendix A: sih
/// (A.1), s1 / k1 / sef / sdf (A.2 - A.4)
pub fn self_test() -> bool {
    let sirk = SAMPLE_SIRK;
    let ltk = SAMPLE_LTK;
    let mut ok = s1(b"SIRKenc") == SAMPLE_S1;
    ok = ok && k1(&ltk, &SAMPLE_S1, b"csis") == SAMPLE_K1;
    ok = ok && sih(&sirk, 0x69f563) == 0x1948da;
    ok = ok && sef(&ltk, &sirk) == SAMPLE_ENC_SIRK && sdf(&ltk, &SAMPLE_ENC_SIRK) == sirk;
    ok && rsi_matches(&sirk, &[0xda, 0x48, 0x19, 0x63, 0xf5, 0x69])
}

// CSIS 1.0.1 appendix A sample data
const SAMPLE_SIRK: Key = hex("457d7d0921a1fd22cecd8c86dd72cccd");
const SAMPLE_LTK: Key = hex("676e1b9bd448696f061ec6223ce5ced9");
const SAMPLE_S1: Key = hex("6901983f18149e823c7d133a7d774572");
const SAMPLE_K1: Key = hex("5277453cc094d982b0e8ee532f2d1f8b");
const SAMPLE_ENC_SIRK: Key = hex("170a3835e13524a07e2562d5f25fd346");

/// 32 hex digits, most significant octet first (compile time only)
const fn hex(s: &str) -> Key {
    const fn nibble(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => panic!("not a hex digit"),
        }
    }
    let b = s.as_bytes();
    assert!(b.len() == 32);
    let mut k = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        k[i] = nibble(b[2 * i]) << 4 | nibble(b[2 * i + 1]);
        i += 1;
    }
    k
}

// ---- AES-128 (FIPS-197), encryption only ----

#[rustfmt::skip]
const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// The 11 round keys
fn expand_key(key: &Key) -> [[u8; 16]; 11] {
    const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];
    let mut w = [[0u8; 4]; 44];
    for (i, word) in w.iter_mut().take(4).enumerate() {
        word.copy_from_slice(&key[4 * i..4 * i + 4]);
    }
    for i in 4..44 {
        let mut t = w[i - 1];
        if i % 4 == 0 {
            t = [SBOX[t[1] as usize] ^ RCON[i / 4 - 1], SBOX[t[2] as usize], SBOX[t[3] as usize], SBOX[t[0] as usize]];
        }
        for j in 0..4 {
            w[i][j] = w[i - 4][j] ^ t[j];
        }
    }
    let mut rk = [[0u8; 16]; 11];
    for (r, k) in rk.iter_mut().enumerate() {
        for c in 0..4 {
            k[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
        }
    }
    rk
}

fn add_round_key(s: &mut [u8; 16], k: &[u8; 16]) {
    for i in 0..16 {
        s[i] ^= k[i];
    }
}

fn sub_bytes(s: &mut [u8; 16]) {
    for b in s.iter_mut() {
        *b = SBOX[*b as usize];
    }
}

/// The state is column-major: s[4 * column + row]
fn shift_rows(s: &mut [u8; 16]) {
    let t = *s;
    for c in 0..4 {
        for r in 1..4 {
            s[4 * c + r] = t[4 * ((c + r) % 4) + r];
        }
    }
}

fn xtime(b: u8) -> u8 {
    (b << 1) ^ if b & 0x80 != 0 { 0x1b } else { 0 }
}

fn mix_columns(s: &mut [u8; 16]) {
    for c in 0..4 {
        let a = [s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]];
        let all = a[0] ^ a[1] ^ a[2] ^ a[3];
        for r in 0..4 {
            s[4 * c + r] = a[r] ^ all ^ xtime(a[r] ^ a[(r + 1) % 4]);
        }
    }
}

/// Subkey derivation of RFC 4493: shift left by one bit, conditional XOR
fn cmac_double(input: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut carry = 0u8;
    for i in (0..16).rev() {
        out[i] = input[i] << 1 | carry;
        carry = input[i] >> 7;
    }
    if input[0] & 0x80 != 0 {
        out[15] ^= 0x87;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_fips197() {
        // FIPS-197 appendix C.1
        let key = hex("000102030405060708090a0b0c0d0e0f");
        let pt = hex("00112233445566778899aabbccddeeff");
        assert_eq!(aes128(&key, &pt), hex("69c4e0d86a7b0430d8cdb78070b4c55a"));
        // FIPS-197 appendix B
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        assert_eq!(aes128(&key, &hex("3243f6a8885a308d313198a2e0370734")), hex("3925841d02dc09fbdc118597196a0b32"));
    }

    #[test]
    fn cmac_rfc4493() {
        let key = hex("2b7e151628aed2a6abf7158809cf4f3c");
        let m: Vec<u8> = [
            hex("6bc1bee22e409f96e93d7e117393172a"),
            hex("ae2d8a571e03ac9c9eb76fac45af8e51"),
            hex("30c81c46a35ce411e5fbc1191a0a52ef"),
            hex("f69f2445df4f9b17ad2b417be66c3710"),
        ]
        .concat();
        // Subkeys (RFC 4493 4. Test Vectors)
        let l = aes128(&key, &[0; 16]);
        assert_eq!(l, hex("7df76b0c1ab899b33e42f047b91b546f"));
        assert_eq!(cmac_double(&l), hex("fbeed618357133667c85e08f7236a8de"));
        assert_eq!(cmac_double(&cmac_double(&l)), hex("f7ddac306ae266ccf90bc11ee46d513b"));
        // Examples 1 - 4: 0, 16, 40 and 64 octets
        assert_eq!(aes_cmac(&key, &[]), hex("bb1d6929e95937287fa37d129b756746"));
        assert_eq!(aes_cmac(&key, &m[..16]), hex("070a16b46b4d4144f79bdd9dd04a287c"));
        assert_eq!(aes_cmac(&key, &m[..40]), hex("dfa66747de9ae63030ca32611497c827"));
        assert_eq!(aes_cmac(&key, &m), hex("51f0bebf7e3b9d92fc49741779363cfe"));
    }

    #[test]
    fn csis_appendix_a() {
        // A.2 s1
        assert_eq!(s1(b"SIRKenc"), SAMPLE_S1);
        // A.3 k1: T = AES-CMAC_SALT(LTK), then AES-CMAC_T("csis")
        assert_eq!(aes_cmac(&SAMPLE_S1, &SAMPLE_LTK), hex("8504ff19dc88a28663f3ccfc5f88c638"));
        assert_eq!(k1(&SAMPLE_LTK, &SAMPLE_S1, b"csis"), SAMPLE_K1);
        // A.4 sef / sdf
        assert_eq!(sef(&SAMPLE_LTK, &SAMPLE_SIRK), SAMPLE_ENC_SIRK);
        assert_eq!(sdf(&SAMPLE_LTK, &SAMPLE_ENC_SIRK), SAMPLE_SIRK);
        // A.1 sih, and the RSI it makes
        assert_eq!(sih(&SAMPLE_SIRK, 0x69f563), 0x1948da);
        assert!(rsi_matches(&SAMPLE_SIRK, &[0xda, 0x48, 0x19, 0x63, 0xf5, 0x69]));
        assert!(!rsi_matches(&SAMPLE_SIRK, &[0xdb, 0x48, 0x19, 0x63, 0xf5, 0x69]));
        assert!(!rsi_matches(&SAMPLE_LTK, &[0xda, 0x48, 0x19, 0x63, 0xf5, 0x69]));
        assert!(self_test());
    }
}
