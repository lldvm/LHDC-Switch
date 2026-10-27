//! State the BTstack thread shares with the callers of [`crate::BtHost`]: what
//! the blocking calls wait for (v1 used a Windows event per wait) and what
//! they read afterwards. One mutex and one condition variable; the BTstack
//! thread holds the lock only to update a few fields.

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use codec::caps::RemoteCaps;

use crate::{ConnectFailure, DiscoveredDevice, InitFailure};

#[derive(Default)]
pub struct State {
    // ---- init ----
    /// When hci_power_control(ON) was called
    pub power_on_at: Option<Instant>,
    /// Events from the controller since power on (tells one that never
    /// answers from one that is slow, e.g. downloading firmware)
    pub controller_events: u32,
    pub init_done: Option<Result<(), InitFailure>>,
    pub hci_ready: bool,
    pub hci_off: bool,

    // ---- one-shot completions, cleared by the caller before each request ----
    pub connect_done: Option<bool>,
    pub stream_done: Option<bool>,
    pub start_done: Option<bool>,
    pub released: bool,
    pub acl_down: bool,
    pub inquiry_done: bool,
    pub probe_answered: bool,
    pub probe_pending: bool,

    /// Set by a user stop: ends waits for connections (stays set across
    /// reconnect attempts until the next user-initiated operation)
    pub cancel: bool,

    // ---- connection ----
    pub connected: bool,
    pub streaming: bool,
    pub disconnect_occurred: bool,
    pub last_connect_failure: Option<ConnectFailure>,
    /// ACL link to the device being connected, and why it went down (0 =
    /// still up / none): a device that ends the link itself vs. a link lost
    pub target_acl_handle: Option<u16>,
    pub target_acl_down_reason: u8,
    /// ACL handle of the A2DP signaling channel
    pub a2dp_con_handle: Option<u16>,
    pub remote_caps: RemoteCaps,
    pub discovered: Vec<DiscoveredDevice>,
    /// Media payload size of the stream (RTP header excluded)
    pub media_mtu: u16,
    /// The stack stopped working (see BtHost::stalled)
    pub stalled: bool,
}

#[derive(Default)]
pub struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    Done,
    Cancelled,
    TimedOut,
}

impl Shared {
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Changes the state and wakes the waiters
    pub fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let r = f(&mut self.lock());
        self.cv.notify_all();
        r
    }

    /// Waits until `done` holds, `timeout` passes or (if `cancellable`) a
    /// cancel is requested
    pub fn wait(&self, timeout: Duration, cancellable: bool, done: impl Fn(&State) -> bool) -> Wait {
        let deadline = Instant::now() + timeout;
        let mut s = self.lock();
        loop {
            if done(&s) {
                return Wait::Done;
            }
            if cancellable && s.cancel {
                return Wait::Cancelled;
            }
            let now = Instant::now();
            if now >= deadline {
                return Wait::TimedOut;
            }
            s = self.cv.wait_timeout(s, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}
