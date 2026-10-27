//! The system timer resolution while LE Audio sends. Windows' default
//! timer ticks every 15.6 ms; the run loop's timers and time_ms() then move
//! in such steps and the ISO send guard fires up to 15 ms late ("late SDU"
//! at the sink). v1 raised it to 1 ms while sending (timeBeginPeriod) and
//! restored it at stop.

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period_ms: u32) -> u32;
    fn timeEndPeriod(period_ms: u32) -> u32;
}

const TIMERR_NOERROR: u32 = 0;

/// Holds a 1 ms system timer resolution until dropped
#[derive(Debug)]
pub struct HighTimerResolution {
    active: bool,
}

impl HighTimerResolution {
    pub fn new() -> HighTimerResolution {
        // SAFETY: timeBeginPeriod takes a plain value; every successful
        // call is paired with timeEndPeriod in Drop
        let active = unsafe { timeBeginPeriod(1) } == TIMERR_NOERROR;
        if !active {
            tracing::warn!(target: "btstack", "timeBeginPeriod(1) failed: LE Audio timers keep the default resolution");
        }
        HighTimerResolution { active }
    }
}

impl Default for HighTimerResolution {
    fn default() -> Self {
        HighTimerResolution::new()
    }
}

impl Drop for HighTimerResolution {
    fn drop(&mut self) {
        if self.active {
            // SAFETY: ends the period begun in new()
            unsafe { timeEndPeriod(1) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begins_and_ends() {
        let r = HighTimerResolution::new();
        assert!(r.active);
        drop(r);
        // Nested holders are counted by Windows
        let a = HighTimerResolution::new();
        let b = HighTimerResolution::new();
        drop(a);
        drop(b);
    }
}
