// Ported from Bluedroid's SBC encoder (sbc_packing.c):
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

//! Quantization and frame packing (EncPacking), with the quantizer of
//! SBC_IS_64_MULT_IN_QUANTIZER (a 32 x 16 bit product built from 16-bit
//! halves) and the header CRC.

const SYNCWORD: u8 = 0x9C;

/// One frame's worth of encoder output to pack
pub(super) struct Frame<'a> {
    pub header: u8,
    pub bitpool: u8,
    /// Joint stereo: the join flags are written
    pub joint: bool,
    pub join: &'a [bool],
    pub channels: usize,
    pub subbands: usize,
    pub blocks: usize,
    pub scale_factor: &'a [i32],
    pub bits: &'a [i32],
    pub sb: &'a [i32],
}

/// Writes MSB first through an 8-bit accumulator, like C's `Temp` (UINT8,
/// so every shift and OR truncates to 8 bits) and `s32PresentBit` (free
/// bits left). A full accumulator is only flushed when the next value comes,
/// so the frame length rule at the end matches C.
struct BitWriter<'a> {
    out: &'a mut [u8],
    pos: usize,
    acc: u8,
    free: u32,
}

impl BitWriter<'_> {
    #[inline(always)]
    fn put(&mut self, value: u32, n: u32) {
        let mut n = n;
        while self.free < n {
            n -= self.free;
            self.acc = ((self.acc as u32) << self.free) as u8 | (value >> n) as u8;
            self.out[self.pos] = self.acc;
            self.pos += 1;
            self.acc = 0;
            self.free = 8;
        }
        self.acc = ((self.acc as u32) << n) as u8 | value as u8;
        self.free -= n;
    }
}

/// EncQuantizer as inlined in EncPacking (SBC_IS_64_MULT_IN_QUANTIZER).
/// The arithmetic is MSVC's: signed products and left shifts wrap,
/// right shifts of signed values are arithmetic.
#[inline(always)]
fn quantize(sb: i32, scf: i32, bits: i32) -> u32 {
    let sf_pow2 = 1u32 << (scf + 1);
    let levels = ((1u32 << bits) - 1) as u16;
    // int + unsigned is computed unsigned, then stored back as SINT32
    let t1 = ((sb >> 2) as u32).wrapping_add(sf_pow2 << 12) as i32;
    let t2 = levels as i32;

    // SBC_MULT_64 by 16-bit halves
    let low = (t1 as u16 as i32).wrapping_mul(t2 as u16 as i32);
    let tv2 = (t1 >> 16).wrapping_mul(t2 as u16 as i32);
    let carry = ((((low as u32) >> 16) & 0xFFFF).wrapping_add((tv2 & 0xFFFF) as u32) >> 16) as i32;
    let low = low.wrapping_add(tv2 << 16);
    let hi = (tv2 >> 16).wrapping_add(carry);

    let shift = (scf + 2) as u32;
    let low1 = ((low >> shift) as u32 & ((1u32 << (32 - shift)) - 1)) as i32;
    let hi1 = hi << (32 - shift);
    (((low1 | hi1) >> 12) as u16) as u32
}

/// Packs one frame into `out`, which holds at least its length; returns the
/// length
pub(super) fn pack(f: &Frame<'_>, out: &mut [u8]) -> usize {
    let nsc = f.channels * f.subbands;

    out[0] = SYNCWORD;
    out[1] = f.header;
    out[2] = f.bitpool;
    let mut w = BitWriter { out, pos: 4, acc: 0, free: 8 };

    if f.joint {
        // 4 subbands: the join nibble shares a byte with the first scale
        // factor; 8 subbands: a byte of its own
        let mut join = 0u32;
        for &j in &f.join[..f.subbands] {
            join = (join << 1) | j as u32;
        }
        if f.subbands == 4 {
            w.acc = join as u8;
            w.free = 4;
        } else {
            w.out[w.pos] = join as u8;
            w.pos += 1;
        }
    }

    // Scale factors: unlike sample bits, a completed byte is written at once
    for &scf in &f.scale_factor[..nsc] {
        w.acc = ((w.acc as u32) << 4) as u8 | scf as u8;
        if w.free == 4 {
            w.free = 8;
            w.out[w.pos] = w.acc;
            w.pos += 1;
            w.acc = 0;
        } else {
            w.free = 4;
        }
    }

    for block in f.sb.chunks_exact(nsc).take(f.blocks) {
        for ((&sb, &scf), &bits) in block.iter().zip(f.scale_factor).zip(f.bits) {
            if bits != 0 {
                w.put(quantize(sb, scf, bits), bits as u32);
            }
        }
    }

    // The last (partial or full) accumulator; with a byte-aligned end and no
    // sample bits this adds a zero byte, as C does
    w.out[w.pos] = ((w.acc as u32) << w.free) as u8;
    let len = w.pos + 1;

    // CRC-8 (polynomial 0x1D, initial 0x0F) over header bytes 1 and 2, the
    // join flags and the scale factors, skipping the CRC byte itself
    let mut crc = 0x0Fu8;
    let full = nsc >> 1;
    feed(&mut crc, out[1], 8);
    feed(&mut crc, out[2], 8);
    for &b in &out[4..4 + full] {
        feed(&mut crc, b, 8);
    }
    if f.joint {
        feed(&mut crc, out[4 + full], f.subbands as u32);
    }
    out[3] = crc;
    len
}

/// Feeds the top `nbits` bits of `byte` into the CRC
fn feed(crc: &mut u8, byte: u8, nbits: u32) {
    for j in (8 - nbits..8).rev() {
        let x = ((*crc >> 7) & 1) ^ ((byte >> j) & 1);
        *crc = (*crc << 1) ^ (x * 0x1D);
    }
}
