//! The LE Audio path allocates nothing per round once a stream runs: the
//! encode side (downmix, resampling, LC3, SDU layout into a spare set, push)
//! and the sending side's queue work (pop, recycle, the silence set on an
//! underrun). Counted by a global allocator, per thread, so the test
//! harness's own threads do not count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{Arc, Mutex};

use le_audio::defs::{LOCATION_FRONT_LEFT, LOCATION_FRONT_RIGHT};
use le_audio::plan::{ChannelPlan, ChannelSource, CisPlan, StreamPlan};
use le_audio::presets::StreamPreset;
use le_audio::sdu_queue::SduQueue;
use pipeline::le::{LeEncoder, silence_set};
use pipeline::{InputFormat, SampleType};

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

fn count() {
    if COUNTING.with(Cell::get) {
        ALLOCATIONS.with(|a| a.set(a.get() + 1));
    }
}

// SAFETY: forwards to the system allocator; the counting touches only
// const-initialized thread locals (no allocation, no reentrancy)
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: same contract as the caller's
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: same contract as the caller's
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: same contract as the caller's
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations_in(f: impl FnOnce()) -> u64 {
    ALLOCATIONS.with(|a| a.set(0));
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    ALLOCATIONS.with(Cell::get)
}

fn plan() -> StreamPlan {
    let ch = |allocation, source| ChannelPlan { allocation, source };
    let cis = |id: u8, channels| CisPlan {
        device_id: u32::from(id),
        ase_id: id,
        cis_id: id,
        acl: 0x40,
        cis_handle: 0x60 + u16::from(id),
        channels,
    };
    StreamPlan {
        preset: StreamPreset {
            name: "48_2_2".into(),
            sampling_hz: 48000,
            frame_us: 10000,
            octets: 100,
            target_latency: 2,
            framing: 0,
            rtn: 13,
            max_transport_latency_ms: 95,
            presentation_delay_us: 40000,
        },
        cig_id: 1,
        phy: 2,
        requested_contexts: 4,
        contexts: 4,
        presentation_delay_us: 40000,
        cis: vec![
            // A 2-channel CIS listed right first, and a 1-channel one
            cis(1, vec![ch(LOCATION_FRONT_RIGHT, ChannelSource::Right), ch(LOCATION_FRONT_LEFT, ChannelSource::Left)]),
            cis(2, vec![ch(0, ChannelSource::Mono)]),
        ],
    }
}

/// 10 ms of 5.1 float capture at 44.1 kHz (downmix and resampling on)
fn capture_block(n0: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(441 * 6 * 4);
    for n in n0..n0 + 441 {
        let x = (n as f32 * 0.142).sin() * 0.3;
        for c in 0..6 {
            out.extend_from_slice(&(x * (c + 1) as f32 / 6.0).to_le_bytes());
        }
    }
    out
}

#[test]
fn le_rounds_do_not_allocate() {
    let plan = plan();
    let input = InputFormat { sample_rate: 44100, channels: 6, sample: SampleType::F32 };
    let mut encoder = LeEncoder::new(&plan, input).unwrap();
    let queue = Arc::new(Mutex::new(SduQueue::new()));
    queue.lock().unwrap().configure(2, silence_set(&plan));

    let blocks: Vec<Vec<u8>> = (0..400).map(|b| capture_block(b * 441)).collect();
    // The capture runs ahead by the prefill (queue depth 2 + 3 sets)
    for block in &blocks[..6] {
        encoder.push(block, &queue);
    }
    let mut sent_bytes = 0usize;
    let mut sent_silence = 0u64;
    // One round: 10 ms of capture encoded, one set sent (the sender gives
    // it back once sent), as the encode and BTstack threads do
    let mut round = |block: &[u8]| {
        encoder.push(block, &queue);
        let mut q = queue.lock().unwrap();
        match q.pop() {
            Some(set) => {
                sent_bytes += set.iter().map(Vec::len).sum::<usize>();
                q.recycle(set);
            }
            None => {
                sent_bytes += q.silence().iter().map(Vec::len).sum::<usize>();
                sent_silence += 1;
                // Logged only on the first underruns (formatting allocates)
                assert!(q.take_underrun().is_none());
            }
        }
    };

    for block in &blocks[6..20] {
        round(block);
    }
    let allocations = allocations_in(|| {
        for block in &blocks[20..] {
            round(block);
        }
    });
    let stats = queue.lock().unwrap().stats();
    assert_eq!(allocations, 0, "allocations in 380 rounds ({stats:?})");
    assert!(stats.sets_pushed >= 399, "{stats:?}");
    assert_eq!((stats.underruns, stats.overflows, sent_silence), (0, 0, 0), "{stats:?}");
    assert_eq!(sent_bytes, 394 * 300);
}
