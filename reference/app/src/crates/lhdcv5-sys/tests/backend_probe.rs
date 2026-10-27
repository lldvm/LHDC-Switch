//! Same deterministic, non-silent 24-bit input and mid-stream rate switches
//! for both native backends. Reports encoded-byte fingerprints and CPU time.
use lhdcv5_sys::{LhdcV5Enc,ffi,BACKEND};
#[test]
fn deterministic_audio_and_rate_switches() {
    for rate in [44100,48000,96000,192000] {
        let mut e=LhdcV5Enc::new().unwrap();
        e.init_encoder(rate,24,ffi::LHDC_QUALITY_LOW,1011,20).unwrap();
        let block=e.block_size().unwrap() as usize;
        let mut pcm=vec![0u8;block*6];let mut out=vec![0u8;8192];
        let mut state=0x12345678u32;
        for quality in [5,6,7,8,6,5] {
            e.set_bitrate_index(quality,false).unwrap();
            let mut hash=0xcbf29ce484222325u64;let mut bytes=0;let mut frames=0;
            let mut timings=Vec::new();
            for _ in 0..400 {
                for sample in pcm.chunks_exact_mut(3) {
                    state=state.wrapping_mul(1664525).wrapping_add(1013904223);
                    sample.copy_from_slice(&((state as i32)>>4).to_le_bytes()[..3]);
                }
                let start=std::time::Instant::now();
                let r=e.encode(&pcm,&mut out).unwrap();
                timings.push(start.elapsed().as_nanos() as u64);
                assert!(r.bytes<=1011);bytes+=r.bytes;frames+=r.frames;
                for &b in &out[..r.bytes] {hash=(hash^u64::from(b)).wrapping_mul(0x100000001b3);}
            }
            timings.sort_unstable();
            println!("PROBE backend={BACKEND:?} rate={rate} quality={quality} bytes={bytes} frames={frames} hash={hash:016x} mean_us={:.1} p95_us={:.1} max_us={:.1}",
                timings.iter().sum::<u64>() as f64/400_000.0,timings[379] as f64/1000.0,timings[399] as f64/1000.0);
            assert!(frames>=396);
        }
    }
}
