//! an adaptive raw mutex

use crate::exclusive_lock::RawExclusiveLock;
use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// an adaptive raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<AdaptiveLock>;
/// an adaptive mutex
pub type Mutex<T> = crate::mutex::Mutex<AdaptiveLock, T>;

/// An adaptive mutex lock backed by `parking_lot_core`
pub struct AdaptiveLock {
    state: AtomicU8,
}

impl AdaptiveLock {
    const LOCK_BIT: u8 = 0b01;
    const PARK_BIT: u8 = 0b10;

    /// Create a new adaptive mutex lock
    pub const fn new() -> Self {
        AdaptiveLock {
            state: AtomicU8::new(0),
        }
    }

    /// Create a new raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    #[cold]
    #[inline(never)]
    fn lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state & Self::LOCK_BIT == 0 {
                match self.state.compare_exchange_weak(
                    state,
                    state | Self::LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return true,
                    Err(x) => state = x,
                }
                continue;
            }

            // If there is no queue, try spinning a few times
            if state & Self::PARK_BIT == 0 && spinwait.spin() {
                state = self.state.load(Ordering::Relaxed);
                continue;
            }

            // Set the parked bit
            if state & Self::PARK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | Self::PARK_BIT,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = x;
                    continue;
                }
            }

            // Park our thread until we are woken up by an unlock
            let addr = self as *const _ as usize;
            let validate = || self.state.load(Ordering::Relaxed) == Self::LOCK_BIT | Self::PARK_BIT;
            let before_sleep = || {};
            let timed_out = |_, was_last_thread| {
                // Clear the parked bit if we were the last parked thread
                if was_last_thread {
                    self.state.fetch_and(!Self::PARK_BIT, Ordering::Relaxed);
                }
            };

            // SAFETY:
            //   * `addr` is an address we control.
            //   * `validate`/`timed_out` does not panic or call into any function of `parking_lot`.
            //   * `before_sleep` does not call `park`, nor does it panic.
            match unsafe {
                parking_lot_core::park(
                    addr,
                    validate,
                    before_sleep,
                    timed_out,
                    DEFAULT_PARK_TOKEN,
                    timeout,
                )
            } {
                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(TOKEN_HANDOFF) => return true,

                // We were unparked normally, try acquiring the lock again
                ParkResult::Unparked(_) => (),

                // The validation function failed, try locking again
                ParkResult::Invalid => (),

                // Timeout expired
                ParkResult::TimedOut => return false,
            }

            // Loop back and try locking again
            spinwait.reset();
            state = self.state.load(Ordering::Relaxed);
        }
    }

    #[cold]
    #[inline(never)]
    fn unlock_slow(&self, force_fair: bool) {
        // Unpark one thread and leave the parked bit set if there might
        // still be parked threads on this address.
        let addr = self as *const _ as usize;
        let callback = |result: UnparkResult| {
            // If we are using a fair unlock then we should keep the
            // mutex locked and hand it off to the unparked thread.
            if result.unparked_threads != 0 && (force_fair || result.be_fair) {
                // Clear the parked bit if there are no more parked
                // threads.
                if !result.have_more_threads {
                    self.state.store(Self::LOCK_BIT, Ordering::Relaxed);
                }
                return TOKEN_HANDOFF;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            if result.have_more_threads {
                self.state.store(Self::PARK_BIT, Ordering::Release);
            } else {
                self.state.store(0, Ordering::Release);
            }
            TOKEN_NORMAL
        };

        // SAFETY:
        //   * `addr` is an address we control.
        //   * `callback` does not panic or call into any function of `parking_lot`.
        unsafe {
            parking_lot_core::unpark_one(addr, callback);
        }
    }

    #[cold]
    fn bump_slow(&self, force_fair: bool) {
        self.unlock_slow(force_fair);
        self.exc_lock();
    }
}

impl crate::Init for AdaptiveLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for AdaptiveLock {}
unsafe impl crate::RawLockInfo for AdaptiveLock {
    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for AdaptiveLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.lock_slow(None);
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);

        (state & Self::LOCK_BIT) == 0
            && self
                .state
                .compare_exchange_weak(
                    state,
                    state | Self::LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        if self
            .state
            .compare_exchange(Self::LOCK_BIT, 0, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            self.unlock_slow(false);
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.bump_slow(false);
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockFair for AdaptiveLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        if self
            .state
            .compare_exchange(Self::LOCK_BIT, 0, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            self.unlock_slow(true);
        }
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.bump_slow(true);
        }
    }
}

impl crate::RawTimedLock for AdaptiveLock {
    type Instant = Instant;
    type Duration = Duration;
}

unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for AdaptiveLock {
    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        if self.exc_try_lock() {
            true
        } else {
            self.lock_slow(Some(instant))
        }
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        if self.exc_try_lock() {
            true
        } else {
            self.lock_slow(Instant::now().checked_add(duration))
        }
    }
}

unsafe impl crate::condvar::Parkable for AdaptiveLock {
    fn mark_parked_if_locked(&self) -> bool {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            if state & Self::LOCK_BIT == 0 {
                return false;
            }
            match self.state.compare_exchange_weak(
                state,
                state | Self::PARK_BIT,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(x) => state = x,
            }
        }
    }

    fn mark_parked(&self) {
        self.state.fetch_or(Self::PARK_BIT, Ordering::Relaxed);
    }
}
