// Ported from Bluedroid's SBC encoder (sbc_analysis.c, sbc_dct.c):
//
//   Copyright (C) 1999-2012 Broadcom Corporation
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at:
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

//! Analysis filter bank: the polyphase window and the fast DCT (Feig and
//! Winograd).
//!
//! This is the configuration BTstack builds: SBC_IS_64_MULT_IN_WINDOW_ACCU
//! off (the window uses only the top 16 bits of each prototype coefficient,
//! 5 taps per row), SBC_FAST_DCT on, SBC_IPAQ_OPT on and
//! SBC_IS_64_MULT_IN_IDCT off (the DCT multiplies a 32-bit value by a Q15
//! constant through 64 bits).

/// ENC_VX_BUFFER_SIZE: the history of both channels in one array, as in C,
/// because the shift points depend on how it is split between them
const VX_LEN: usize = 8 * 10 * 2 + 64;

/// The window coefficients are signed 16-bit; C writes them as hex and
/// casts to SINT16
const fn c16(v: u16) -> i32 {
    v as i16 as i32
}

/// Rows 1..=3 of the 4-subband window (row 0 and 4 are symmetric and
/// written out in `window4`)
const W4: [[i32; 5]; 3] = [
    [c16(0x0012), c16(0x029E), c16(0x18F5), c16(0xF610), c16(0xFF9C)],
    [c16(0x0031), c16(0x03B2), c16(0x1F91), c16(0xFC50), c16(0x003D)],
    [c16(0x005A), c16(0x041F), c16(0x2413), c16(0x00C9), c16(0x0080)],
];

/// Rows 1..=7 of the 8-subband window (row 0 and 8 in `window8`)
const W8: [[i32; 5]; 7] = [
    [c16(0x0005), c16(0x0107), c16(0x0A9F), c16(0xF931), c16(0xFF8D)],
    [c16(0x000B), c16(0x0157), c16(0x0C7D), c16(0xFB00), c16(0xFFCA)],
    [c16(0x0012), c16(0x01A2), c16(0x0E3C), c16(0xFCA8), c16(0xFFFA)],
    [c16(0x001B), c16(0x01E0), c16(0x0FC7), c16(0xFE20), c16(0x001E)],
    [c16(0x0025), c16(0x0209), c16(0x110F), c16(0xFF5F), c16(0x0035)],
    [c16(0x0030), c16(0x0214), c16(0x1204), c16(0x0060), c16(0x0041)],
    [c16(0x003A), c16(0x01F6), c16(0x129C), c16(0x0122), c16(0x0045)],
];

const COS_PI_4: i64 = 0x5a82;
const COS_PI_8: i64 = 0x7641;
const COS_3PI_8: i64 = 0x30fb;
const COS_PI_16: i64 = 0x7d8a;
const COS_3PI_16: i64 = 0x6a6d;
const COS_5PI_16: i64 = 0x471c;
const COS_7PI_16: i64 = 0x18f8;

/// SBC_MULT_32_16_SIMPLIFIED with SBC_IPAQ_OPT: the operand is a 32-bit
/// expression (already wrapped), widened, multiplied and shifted in 64 bits;
/// the cast back truncates.
#[inline(always)]
fn mult(c: i64, v: i32) -> i32 {
    ((c * v as i64) >> 15) as i32
}

/// The input history and the shift bookkeeping (SbcAnalysisInit state)
pub(super) struct Analysis {
    x: [i16; VX_LEN],
    shift_counter: usize,
    /// EncMaxShiftCounter
    max_shift: usize,
    /// Distance between the two channels' histories in `x` (Offset2)
    ch_stride: usize,
    dcty: [i32; 16],
}

impl Analysis {
    pub(super) fn new(subbands: usize, channels: usize) -> Analysis {
        // SBC_Encoder_Init: as many blocks as fit into the buffer beyond the
        // 10 * subbands window, rounded so that the counter (stepping by
        // `subbands`) hits it exactly
        let max_shift = match (subbands, channels) {
            (4, 1) => ((VX_LEN - 4 * 10) >> 2) << 2,
            (4, _) => ((VX_LEN - 4 * 10 * 2) >> 3) << 2,
            (_, 1) => ((VX_LEN - 8 * 10) >> 3) << 3,
            _ => ((VX_LEN - 8 * 10 * 2) >> 4) << 3,
        };
        Analysis { x: [0; VX_LEN], shift_counter: 0, max_shift, ch_stride: max_shift + 10 * subbands, dcty: [0; 16] }
    }

    /// SbcAnalysisFilter4 / SbcAnalysisFilter8: `pcm` holds blocks *
    /// subbands samples per channel, interleaved; `sb` receives them per
    /// block, channel, subband
    pub(super) fn filter(&mut self, pcm: &[i16], subbands: usize, channels: usize, blocks: usize, sb: &mut [i32]) {
        let stride = self.ch_stride;
        for (pcm, sb) in
            pcm.chunks_exact(subbands * channels).zip(sb.chunks_exact_mut(subbands * channels)).take(blocks)
        {
            let offset = self.max_shift - self.shift_counter;
            // Newest sample last in time goes lowest in the history
            for (i, frame) in pcm.chunks_exact(channels).enumerate() {
                let pos = offset + subbands - 1 - i;
                for (ch, &s) in frame.iter().enumerate() {
                    self.x[ch * stride + pos] = s;
                }
            }
            for (ch, out) in sb.chunks_exact_mut(subbands).enumerate() {
                let base = ch * stride + offset;
                if subbands == 4 {
                    window4(self.x[base..base + 40].try_into().unwrap(), &mut self.dcty);
                    idct4(&self.dcty, out);
                } else {
                    window8(self.x[base..base + 80].try_into().unwrap(), &mut self.dcty);
                    idct8(&self.dcty, out);
                }
            }
            if self.shift_counter >= self.max_shift {
                // SHIFTUP_X4 / SHIFTUP_X8: C moves 32-bit words downwards
                // from the top, the source lying below the destination, which
                // is a memmove of 9 * subbands samples per channel
                let len = 9 * subbands;
                let dst = self.max_shift + subbands;
                let src = dst - subbands - self.shift_counter;
                for ch in 0..channels {
                    let o = ch * stride;
                    self.x.copy_within(o + src..o + src + len, o + dst);
                }
                self.shift_counter = 0;
            } else {
                self.shift_counter += subbands;
            }
        }
    }
}

// The window sums stay below 2^29 for any 16-bit input (the absolute
// coefficients of a row add up to at most 11592, times 32768), so plain i32
// arithmetic cannot overflow here.

fn window4(x: &[i16; 40], y: &mut [i32; 16]) {
    let s = |i: usize| x[i] as i32;
    y[0] = 0x0166 * (s(8) - s(32)) + 0x115B * (s(16) - s(24));
    for k in 1..4 {
        let c = &W4[k - 1];
        let mut t = 0;
        let mut t2 = 0;
        for (j, &c) in c.iter().enumerate() {
            t += c * s(8 * j + k);
            t2 += c * s(8 * (4 - j) + 8 - k);
        }
        y[k] = t;
        y[8 - k] = t2;
    }
    y[4] = 0x007E * (s(4) + s(36)) + 0x0350 * (s(12) + s(28)) + 0x25AC * s(20);
}

fn window8(x: &[i16; 80], y: &mut [i32; 16]) {
    let s = |i: usize| x[i] as i32;
    y[0] = 0x00B9 * (s(16) - s(64)) + 0x08B4 * (s(32) - s(48));
    for k in 1..8 {
        let c = &W8[k - 1];
        let mut t = 0;
        let mut t2 = 0;
        for (j, &c) in c.iter().enumerate() {
            t += c * s(16 * j + k);
            t2 += c * s(16 * (4 - j) + 16 - k);
        }
        y[k] = t;
        y[16 - k] = t2;
    }
    y[8] = 0x0042 * (s(8) + s(72)) + 0x01A8 * (s(24) + s(56)) + 0x12CF * s(40);
}

// The DCTs use wrapping arithmetic: the shifted-up odd terms of the 8-point
// transform could exceed 32 bits for extreme (non-audio) input, and the C
// code then wraps (MSVC x64 emits plain 32-bit add/sub/shl). `>>` on i32 is
// arithmetic, as MSVC's on negative values.

/// SBC_FastIDCT4 (y[6] is computed by the window but not used, as in C)
fn idct4(y: &[i32; 16], out: &mut [i32]) {
    let x2 = y[2] >> 1;
    let temp = y[0].wrapping_add(y[4]);
    let mut t0 = mult(COS_PI_4 >> 1, temp);
    let t1 = x2.wrapping_sub(t0);
    t0 = t0.wrapping_add(x2);
    let temp = y[1].wrapping_add(y[3]);
    let t3 = mult(COS_3PI_8 >> 1, temp);
    let t2 = mult(COS_PI_8 >> 1, temp);
    let temp = y[5].wrapping_sub(y[7]);
    let t5 = mult(COS_3PI_8 >> 1, temp);
    let t4 = mult(COS_PI_8 >> 1, temp);
    let t6 = t2.wrapping_add(t5);
    let t7 = t3.wrapping_sub(t4);
    out[0] = t0.wrapping_add(t6);
    out[1] = t1.wrapping_add(t7);
    out[2] = t1.wrapping_sub(t7);
    out[3] = t0.wrapping_sub(t6);
}

/// SBC_FastIDCT8 (y[12] is not used, as in C)
fn idct8(y: &[i32; 16], out: &mut [i32]) {
    let mut x0 = mult(COS_PI_4, y[4]);
    let mut x1 = y[3].wrapping_add(y[5]) >> 1;
    let mut x2 = y[2].wrapping_add(y[6]) >> 1;
    let mut x3 = y[1].wrapping_add(y[7]) >> 1;
    let mut x4 = y[0].wrapping_add(y[8]) >> 1;
    let mut x5 = y[9].wrapping_sub(y[15]) >> 1;
    let mut x6 = y[10].wrapping_sub(y[14]) >> 1;
    let mut x7 = y[11].wrapping_sub(y[13]) >> 1;

    // 2-point IDCT of x0 and x4
    let temp = x0;
    x0 = mult(COS_PI_4, x0.wrapping_add(x4));
    x4 = mult(COS_PI_4, temp.wrapping_sub(x4));

    // 2-point IDCT of x2 and x6 with post-multiplication
    x2 = x2.wrapping_sub(x6);
    x6 <<= 1;
    x6 = mult(COS_PI_4, x6);
    let temp = x2;
    x2 = mult(COS_PI_8, x2.wrapping_add(x6));
    x6 = mult(COS_3PI_8, temp.wrapping_sub(x6));

    let even = [x0.wrapping_add(x2), x4.wrapping_add(x6), x4.wrapping_sub(x6), x0.wrapping_sub(x2)];

    // Odd part: rearrangement of x1, x3, x5, x7
    x7 <<= 1;
    x5 = (x5 << 1).wrapping_sub(x7);
    x3 = (x3 << 1).wrapping_sub(x5);
    x1 = x1.wrapping_sub(x3 >> 1);

    x5 = mult(COS_PI_4, x5);
    let temp = x1;
    x1 = x1.wrapping_add(x5);
    x5 = temp.wrapping_sub(x5);

    x3 = x3.wrapping_sub(x7);
    x7 <<= 1;
    x7 = mult(COS_PI_4, x7);

    let temp = x3;
    x3 = mult(COS_PI_8, x3.wrapping_add(x7));
    x7 = mult(COS_3PI_8, temp.wrapping_sub(x7));

    let odd = [
        mult(COS_PI_16, x1.wrapping_add(x3)),
        mult(COS_3PI_16, x5.wrapping_add(x7)),
        mult(COS_5PI_16, x5.wrapping_sub(x7)),
        mult(COS_7PI_16, x1.wrapping_sub(x3)),
    ];

    for i in 0..4 {
        out[i] = even[i].wrapping_add(odd[i]);
        out[7 - i] = even[i].wrapping_sub(odd[i]);
    }
}
