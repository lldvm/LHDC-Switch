//! LHDC Switch adaptive policy. Time based, independent of packet size/rate.
use std::time::Duration;

pub(crate) struct Abr {
    pub table: Vec<u32>,
    pub index: usize,
    pub start: usize,
    clear_since: Duration,
    congested_since: Option<Duration>,
    next_probe: Duration,
    probe_delay: Duration,
    /// Failed probes pause only that stage, never lower the session ceiling.
    blocked_probe: Option<(usize, Duration)>,
    next_reduction: Duration,
    last_raise: Option<(usize, Duration)>,
}

impl Abr {
    pub fn new(rates: &[u32], min: usize, max: usize, cap: u32) -> Self {
        let ceiling = rates[max].min(cap).max(rates[min]);
        let table: Vec<_> = rates.iter().enumerate()
            .filter(|(i, r)| *i >= min && *i <= max && **r <= ceiling && **r >= 160)
            .map(|(_, r)| *r).collect();
        let table = if table.is_empty() { vec![rates[min]] } else { table };
        let start = table.iter().rposition(|r| *r <= 400).unwrap_or(0);
        Self { table, index: start, start, clear_since: Duration::ZERO,
            congested_since: None, next_probe: Duration::from_secs(5), probe_delay: Duration::from_secs(15),
            blocked_probe: None, next_reduction: Duration::ZERO, last_raise: None }
    }

    // A few packets are normal with 10 ms capture bursts. Sustained backlog
    // reduces rate before the 64-packet ring fills; severe backlog acts at once.
    pub fn target(&mut self, queue: u32, now: Duration) -> Option<usize> {
        if queue >= 4 {
            self.clear_since = now;
            if now < Duration::from_secs(2) && queue < 24 { return None; }
            let since = *self.congested_since.get_or_insert(now);
            if self.index > 0 && now >= self.next_reduction
                && (queue >= 24 || now.saturating_sub(since) >= Duration::from_millis(60)) {
                if let Some((raised, at)) = self.last_raise.take() {
                    if raised == self.index && now.saturating_sub(at) <= Duration::from_secs(6) {
                        self.blocked_probe = Some((raised,now + Duration::from_secs(60)));
                    }
                }
                self.congested_since = Some(now);
                let target=if queue >= 24 { self.start.min(self.index - 1) } else { self.index.saturating_sub(2) };
                self.next_reduction=now+Duration::from_millis(250);
                // Recover ordinary audio quality promptly after the link clears;
                // a series of downshifts must not leave a 60 s floor penalty.
                self.next_probe = now + if target < self.start {Duration::from_secs(10)} else {self.probe_delay};
                self.probe_delay = (self.probe_delay * 2).min(Duration::from_secs(60));
                return Some(target);
            }
        } else {
            self.congested_since = None;
            if queue > 1 { self.clear_since = now; }
            let hold = if self.table.get(self.index + 1).is_some_and(|r| *r >= 900) { 20 }
                else if self.index < self.start {3} else {5};
            let probe_blocked=self.blocked_probe.is_some_and(|(stage,until)| self.index+1>=stage && now<until);
            if queue <= 1 && self.index + 1 < self.table.len() && !probe_blocked && now >= self.next_probe
                && now.saturating_sub(self.clear_since) >= Duration::from_secs(hold) {
                self.clear_since = now;
                self.next_probe = now + Duration::from_secs(if self.index < self.start {3} else {5});
                self.probe_delay = Duration::from_secs(15);
                self.last_raise = Some((self.index + 1, now));
                return Some(self.index + 1);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const R: [u32; 9] = [64,160,192,256,320,400,500,900,1000];
    #[test]
    fn climbs_to_requested_ceiling_and_stays_there() {
        for cap in [400,500,900,1000] {
            let mut a = Abr::new(&R,0,8,cap);
            assert_eq!(a.table[a.index],400);
            for t in 0..100 { if let Some(i)=a.target(0,Duration::from_secs(t)) { a.index=i; } }
            assert_eq!(a.table[a.index],cap);
        }
    }
    #[test]
    fn honors_peer_and_mtu_limits_and_minimum() {
        assert_eq!(*Abr::new(&R,0,5,1000).table.last().unwrap(),400);
        assert_eq!(*Abr::new(&R,0,7,1000).table.last().unwrap(),900);
        assert_eq!(Abr::new(&R,5,8,160).table,vec![400]);
        let r44=[64,160,192,240,320,400,480,900,1000];
        assert_eq!(*Abr::new(&r44,0,8,500).table.last().unwrap(),480);
    }
    #[test]
    fn backs_off_rapidly_then_waits_before_retry() {
        let mut a=Abr::new(&R,0,8,1000); a.index=7;
        let t=Duration::from_secs(20);
        let i=a.target(24,t).unwrap(); assert_eq!(a.table[i],400); a.index=i;
        assert_eq!(a.target(0,t+Duration::from_secs(5)),None);
        assert!(a.target(0,t+Duration::from_secs(15)).is_some());
        assert_eq!(a.target(4,t+Duration::from_secs(16)),None);
        assert!(a.target(4,t+Duration::from_millis(16060)).is_some());
    }
    #[test]
    fn tolerates_short_capture_bursts() {
        let mut a=Abr::new(&R,0,8,1000);
        assert_eq!(a.target(5,Duration::from_millis(10)),None);
        assert_eq!(a.target(0,Duration::from_millis(20)),None);
        assert_eq!(a.target(5,Duration::from_millis(30)),None);
    }
    #[test]
    fn failed_probe_retries_after_cooldown_instead_of_permanently_capping_stream() {
        let mut a=Abr::new(&R,0,8,1000);
        for t in [5,25] { let i=a.target(0,Duration::from_secs(t)).unwrap(); a.index=i; }
        assert_eq!(a.table[a.index],900);
        let i=a.target(24,Duration::from_secs(26)).unwrap(); a.index=i;
        for t in 27..86 { if let Some(i)=a.target(0,Duration::from_secs(t)) { a.index=i; } }
        assert_eq!(a.table[a.index],500);
        for t in 86..180 { if let Some(i)=a.target(0,Duration::from_secs(t)) { a.index=i; } }
        assert_eq!(a.table[a.index],1000);
    }
    #[test]
    fn reductions_wait_for_the_previous_change_and_floor_recovers() {
        let mut a=Abr::new(&R,0,8,900);a.index=6;
        let t=Duration::from_secs(40);
        let i=a.target(24,t).unwrap();a.index=i;
        assert_eq!(a.target(24,t+Duration::from_millis(20)),None);
        for dt in [250,500,750,1000] { if let Some(i)=a.target(24,t+Duration::from_millis(dt)) {a.index=i;} }
        assert_eq!(a.table[a.index],160);
        for seconds in 42..76 {if let Some(i)=a.target(0,Duration::from_secs(seconds)){a.index=i;}}
        assert!(a.table[a.index]>=500);
        for seconds in 76..110 {if let Some(i)=a.target(0,Duration::from_secs(seconds)){a.index=i;}}
        assert_eq!(a.table[a.index],900);
    }
    #[test]
    fn startup_grace_does_not_ignore_a_full_queue() {
        let mut a=Abr::new(&R,0,8,900);
        assert_eq!(a.target(7,Duration::from_millis(100)),None);
        assert!(a.target(64,Duration::from_millis(200)).is_some());
    }
}
