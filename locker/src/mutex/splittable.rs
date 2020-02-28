use crate::exclusive_lock::RawExclusiveLock;
use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

pub struct RawLock {
    state: AtomicUsize,
}

impl RawLock {
    const LOCK_INC: usize = 0b10;
    const LOCK_BITS: usize = !Self::PARK_BIT;
    const PARK_BIT: usize = 0b01;

    pub const fn new() -> Self {
        RawLock {
            state: AtomicUsize::new(0),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }

    #[cold]
    #[inline(never)]
    fn lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state & Self::LOCK_BITS == 0 {
                if let Some(new_state) = state.checked_add(Self::LOCK_INC) {
                    match self.state.compare_exchange_weak(
                        state,
                        new_state,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => return true,
                        Err(x) => state = x,
                    }

                    continue;
                }
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
            let validate = || {
                // check if locked and parked bit is set

                self.state.load(Ordering::Relaxed) != 0
            };
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
                    self.state.fetch_and(!Self::PARK_BIT, Ordering::Relaxed);
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
        self.uniq_lock();
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();
    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        if !self.uniq_try_lock() {
            self.lock_slow(None);
        }
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);

        (state & Self::LOCK_BITS) == 0
            && self
                .state
                .compare_exchange_weak(
                    state & Self::PARK_BIT,
                    state | Self::LOCK_INC,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        let state = self.state.load(Ordering::Relaxed) & Self::LOCK_BITS;

        debug_assert_ne!(state, 0, "uniq_unlock was called with an unlocked mutex");

        if self
            .state
            .compare_exchange(
                state,
                state - Self::LOCK_INC,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            self.unlock_slow(false);
        }
    }

    #[inline]
    unsafe fn uniq_bump(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.bump_slow(false)
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockFair for RawLock {
    #[inline]
    unsafe fn uniq_unlock_fair(&self) {
        let state = self.state.load(Ordering::Relaxed) & Self::LOCK_BITS;

        debug_assert_ne!(state, 0, "uniq_unlock was called with an unlocked mutex");

        if self
            .state
            .compare_exchange(
                state,
                state - Self::LOCK_INC,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            self.unlock_slow(true);
        }
    }

    #[inline]
    unsafe fn uniq_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.bump_slow(true)
        }
    }
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for RawLock {
    unsafe fn uniq_split(&self) {
        self.state.fetch_and(Self::LOCK_INC, Ordering::Relaxed);
    }
}

// TODO: fix performace bug
unsafe impl crate::condvar::Parkable for RawLock {
    fn mark_parked_if_locked(&self) -> bool {
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            if state & Self::LOCK_BITS == 0 {
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
