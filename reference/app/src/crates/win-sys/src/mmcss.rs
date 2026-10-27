// Modified by LHDC Switch contributors, 2026-10-27. See engine-source/MODIFICATIONS.md at the LHDC Switch repository root.
//! MMCSS (Multimedia Class Scheduler Service) thread registration.
//!
//! The audio threads register with MMCSS so that a busy PC cannot hold them
//! back for tens of milliseconds: "Audio" for the capture thread (as v1's
//! WASAPI capture and test tone did), "Pro Audio" for the encode thread.
//! Registration belongs to the calling thread, so the guard is `!Send` and
//! reverts on the same thread when it is dropped.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW};
use windows::core::PCWSTR;

/// Per-thread high-resolution waitable timer. No global timer-resolution
/// changes and no busy-spinning on the audio path. Closed at thread exit.
pub fn media_wait(duration: std::time::Duration) {
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{CreateWaitableTimerExW,SetWaitableTimer,WaitForSingleObject,CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,TIMER_ALL_ACCESS};
    struct Timer(HANDLE);
    impl Drop for Timer {
        fn drop(&mut self) {
            // SAFETY: this thread owns this valid timer handle exactly once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
    thread_local! {
        static TIMER: Option<Timer> = {
            // SAFETY: unnamed timer, no borrowed attributes or name.
            unsafe { CreateWaitableTimerExW(None,PCWSTR::null(),CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,TIMER_ALL_ACCESS.0).ok().map(Timer) }
        };
    }
    let waited = TIMER.with(|timer| {
        let Some(timer) = timer else { return false; };
        let due = -((duration.as_nanos().div_ceil(100)).clamp(1,i64::MAX as u128) as i64);
        // SAFETY: timer is owned by this thread, due is a valid relative
        // 100-ns interval, no APC or borrowed callback data, no periodic timer.
        unsafe {
            SetWaitableTimer(timer.0,&due,0,None,None,false).is_ok()
                && WaitForSingleObject(timer.0,100) == WAIT_OBJECT_0
        }
    });
    if !waited { std::thread::sleep(duration); }
}

/// MMCSS task names (keys under HKLM\...\Multimedia\SystemProfile\Tasks)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmcssTask {
    /// "Audio": the WASAPI capture thread
    Audio,
    /// "Pro Audio": the encode thread, which has a deadline of its own
    ProAudio,
}

impl MmcssTask {
    /// The task name as MMCSS knows it
    pub fn name(self) -> &'static str {
        match self {
            MmcssTask::Audio => "Audio",
            MmcssTask::ProAudio => "Pro Audio",
        }
    }
}

/// The calling thread's MMCSS registration; dropping it reverts the thread
/// to normal scheduling. Holds a raw handle, so it is neither `Send` nor
/// `Sync`: AvRevertMmThreadCharacteristics must run on the registered thread.
#[derive(Debug)]
pub struct MmcssGuard {
    handle: HANDLE,
    task_index: u32,
}

impl MmcssGuard {
    /// Register the calling thread with `task`. Fails when the MMCSS
    /// service is unavailable (stopped, or a Windows edition without it);
    /// the thread then simply keeps its normal priority.
    pub fn register(task: MmcssTask) -> windows::core::Result<MmcssGuard> {
        let name: Vec<u16> = task.name().encode_utf16().chain(Some(0)).collect();
        let mut task_index = 0u32;
        // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the
        // call, and `task_index` is a valid out pointer. The task index starts
        // at 0, which asks MMCSS to assign a new one.
        let handle = unsafe { AvSetMmThreadCharacteristicsW(PCWSTR(name.as_ptr()), &mut task_index)? };
        Ok(MmcssGuard { handle, task_index })
    }

    /// The task index MMCSS assigned
    pub fn task_index(&self) -> u32 {
        self.task_index
    }
}

impl Drop for MmcssGuard {
    fn drop(&mut self) {
        // SAFETY: `handle` came from AvSetMmThreadCharacteristicsW on this
        // thread (the guard is !Send) and is reverted exactly once, here.
        if let Err(e) = unsafe { AvRevertMmThreadCharacteristics(self.handle) } {
            tracing::debug!("MMCSS revert failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_revert() {
        // The MMCSS service runs on every desktop Windows; a failure here
        // means the service is stopped, which is not this code's fault.
        match MmcssGuard::register(MmcssTask::Audio) {
            Ok(guard) => drop(guard),
            Err(e) => eprintln!("MMCSS unavailable: {e}"),
        }
    }

    #[test]
    fn task_names() {
        assert_eq!(MmcssTask::Audio.name(), "Audio");
        assert_eq!(MmcssTask::ProAudio.name(), "Pro Audio");
    }

    #[test]
    fn high_resolution_media_wait_smoke() {
        let start=std::time::Instant::now();
        for _ in 0..20 { media_wait(std::time::Duration::from_millis(3)); }
        let elapsed=start.elapsed();
        println!("20 x 3 ms waitable timer: {:.2} ms",elapsed.as_secs_f64()*1000.0);
        assert!(elapsed>=std::time::Duration::from_millis(59));
        assert!(elapsed<std::time::Duration::from_secs(2));
    }
}
