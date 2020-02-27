use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type Once = crate::once::Once<RawLock>;
pub type OnceCell<T> = crate::once::OnceCell<RawLock, T>;
pub type Lazy<T, F> = crate::once::Lazy<RawLock, T, F, crate::once::Panic>;
pub type RertyLazy<T, F> = crate::once::Lazy<RawLock, T, F, crate::once::Retry>;

pub struct RawLock {
    state: AtomicU8,
}

unsafe impl crate::once::Finish for RawLock {
    #[inline]
    fn is_done(&self) -> bool {
        self.state.load(Ordering::Relaxed) & Self::DONE_BIT != 0
    }

    #[inline]
    fn mark_done(&self) {
        self.state.fetch_or(Self::DONE_BIT, Ordering::Relaxed);
    }

    #[inline]
    fn get_and_mark_poisoned(&self) -> bool {
        let state = self.state.fetch_or(Self::POISON_BIT, Ordering::Relaxed);

        state & Self::POISON_BIT != 0
    }
}

impl RawLock {
    const LOCK_BIT: u8 = 0b0001;
    const PARK_BIT: u8 = 0b0010;
    const DONE_BIT: u8 = 0b0100;
    const POISON_BIT: u8 = 0b1000;

    pub const fn new() -> Self {
        RawLock {
            state: AtomicU8::new(0),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }

    pub const fn once() -> Once {
        unsafe { Once::from_raw(Self::new()) }
    }

    pub const fn once_cell<T>() -> OnceCell<T> {
        unsafe { OnceCell::from_once(Self::once()) }
    }

    pub const fn lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }

    pub const fn retry_lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
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
            let validate = || {
                (self.state.load(Ordering::Relaxed) & !Self::DONE_BIT)
                    == Self::LOCK_BIT | Self::PARK_BIT
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
                self.state.fetch_and(!Self::PARK_BIT, Ordering::Release);
            } else {
                self.state
                    .fetch_and(!(Self::LOCK_BIT | Self::PARK_BIT), Ordering::Release);
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
}

unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();
    type UniqueGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::unique_lock::RawUniqueLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        if !self.uniq_try_lock() {
            self.lock_slow(None);
        }
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
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

    /// # Safety
    ///
    /// This unique lock must be locked before calling this function
    #[inline]
    unsafe fn uniq_unlock(&self) {
        if self
            .state
            .compare_exchange(Self::LOCK_BIT, 0, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            self.unlock_slow(false);
        }
    }
}
