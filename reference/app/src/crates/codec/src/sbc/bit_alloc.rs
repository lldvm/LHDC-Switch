// Ported from Bluedroid's SBC encoder (sbc_enc_bit_alloc_mono.c,
// sbc_enc_bit_alloc_ste.c):
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

//! Bit allocation. Mono and dual channel allocate per channel, stereo and
//! joint stereo over both channels together. The two C functions differ in
//! small ways (the end-of-slicing test, the order of the final passes) that
//! change the output, so they are kept apart here as well.

/// sbc_enc_as16Offset4 / sbc_enc_as16Offset8, by sampling frequency index
const OFFSET4: [[i32; 4]; 4] = [[-1, 0, 0, 0], [-2, 0, 0, 1], [-2, 0, 0, 1], [-2, 0, 0, 1]];
const OFFSET8: [[i32; 8]; 4] =
    [[-2, 0, 0, 0, 0, 0, 0, 1], [-3, 0, 0, 0, 0, 0, 1, 2], [-4, 0, 0, 0, 0, 0, 1, 2], [-4, 0, 0, 0, 0, 0, 1, 2]];

/// What the allocation needs besides the scale factors
#[derive(Clone, Copy)]
pub(super) struct AllocParams {
    pub subbands: usize,
    pub freq_index: usize,
    pub snr: bool,
    pub bitpool: i32,
}

/// Loudness bit need of one channel's subbands, written to `out`; returns
/// its maximum (not below 0, as C starts the search at 0)
fn loudness(scf: &[i32], freq_index: usize, out: &mut [i32]) -> i32 {
    let offsets: &[i32] = if scf.len() == 4 { &OFFSET4[freq_index] } else { &OFFSET8[freq_index] };
    let mut max = 0;
    for ((o, &s), &off) in out.iter_mut().zip(scf).zip(offsets) {
        *o = if s == 0 {
            -5
        } else {
            let l = s - off;
            if l > 0 { l >> 1 } else { l }
        };
        max = max.max(*o);
    }
    max
}

/// One slice step: the bits the subbands between `slice + 1` and
/// `slice + 15` take (2 for the one just reaching 1)
fn slice_count(bitneed: &[i32], slice: i32) -> i32 {
    bitneed
        .iter()
        .map(|&bn| match bn - slice {
            1 => 2,
            2..=15 => 1,
            _ => 0,
        })
        .sum()
}

fn initial_bits(bitneed: &[i32], slice: i32, bits: &mut [i32]) {
    for (b, &bn) in bits.iter_mut().zip(bitneed) {
        *b = if bn < slice + 2 { 0 } else { (bn - slice).min(16) };
    }
}

/// sbc_enc_bit_alloc_mono (mono and dual channel)
pub(super) fn mono(p: AllocParams, channels: usize, scf: &[i32; 16], scratch: &mut [i32; 16], bits: &mut [i32; 16]) {
    let n = p.subbands;
    for ch in 0..channels {
        let r = ch * n..(ch + 1) * n;
        let (bitneed, max_bitneed): (&[i32], i32) = if p.snr {
            let bn = &scf[r.clone()];
            (bn, bn.iter().fold(0, |m, &v| m.max(v)))
        } else {
            let max = loudness(&scf[r.clone()], p.freq_index, &mut scratch[r.clone()]);
            (&scratch[r.clone()], max)
        };

        let mut slice = max_bitneed + 1;
        let mut count = p.bitpool;
        let mut sc = 0;
        loop {
            slice -= 1;
            count -= sc;
            sc = slice_count(bitneed, slice);
            if count - sc <= 0 {
                break;
            }
        }
        // C tests the count before taking the last slice (the stereo version
        // tests the remainder); kept as is, it decides the output
        if count == 0 {
            count -= sc;
            slice -= 1;
        }

        let bits = &mut bits[r];
        initial_bits(bitneed, slice, bits);

        let mut sb = 0;
        while count > 0 && sb < n {
            if bits[sb] >= 2 && bits[sb] < 16 {
                bits[sb] += 1;
                count -= 1;
            } else if bitneed[sb] == slice + 1 && count > 1 {
                bits[sb] = 2;
                count -= 2;
            }
            sb += 1;
        }
        sb = 0;
        while count > 0 && sb < n {
            if bits[sb] < 16 {
                bits[sb] += 1;
                count -= 1;
            }
            sb += 1;
        }
    }
}

/// sbc_enc_bit_alloc_ste (stereo and joint stereo). `max_bitneed` is the
/// largest scale factor the encoder saw, used for SNR as C does.
pub(super) fn stereo(p: AllocParams, max_bitneed: i32, scf: &[i32; 16], scratch: &mut [i32; 16], bits: &mut [i32; 16]) {
    let n = p.subbands;
    let (bitneed, max_bitneed): (&[i32], i32) = if p.snr {
        (&scf[..2 * n], max_bitneed)
    } else {
        let m0 = loudness(&scf[..n], p.freq_index, &mut scratch[..n]);
        let m1 = loudness(&scf[n..2 * n], p.freq_index, &mut scratch[n..2 * n]);
        (&scratch[..2 * n], m0.max(m1))
    };

    let mut slice = max_bitneed + 1;
    let mut count = p.bitpool;
    let mut sc = 0;
    loop {
        slice -= 1;
        count -= sc;
        sc = slice_count(bitneed, slice);
        if count - sc <= 0 {
            break;
        }
    }
    if count - sc == 0 {
        count -= sc;
        slice -= 1;
    }

    let bits = &mut bits[..2 * n];
    initial_bits(bitneed, slice, bits);

    // The leftovers go to the subbands alternating between the channels:
    // left 0, right 0, left 1, ...
    let mut ch = 0;
    let mut sb = 0;
    while count > 0 && sb < n {
        let i = ch * n + sb;
        if bits[i] >= 2 && bits[i] < 16 {
            bits[i] += 1;
            count -= 1;
        } else if bitneed[i] == slice + 1 && count > 1 {
            bits[i] = 2;
            count -= 2;
        }
        if ch == 1 {
            ch = 0;
            sb += 1;
        } else {
            ch = 1;
        }
    }
    ch = 0;
    sb = 0;
    while count > 0 && sb < n {
        let i = ch * n + sb;
        if bits[i] < 16 {
            bits[i] += 1;
            count -= 1;
        }
        if ch == 1 {
            ch = 0;
            sb += 1;
        } else {
            ch = 1;
        }
    }
}
