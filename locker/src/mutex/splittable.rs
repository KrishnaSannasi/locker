//! a splittable lock

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

/// a splittable raw mutex
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
pub type RawMutex = crate::mutex::raw::Mutex<SplitLock>;

/// a splittable mutex
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
pub type Mutex<T> = crate::mutex::Mutex<SplitLock, T>;

const PARK_BIT: usize = 0b1;
const INC: usize = 0b10;

/// a splittable lock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
pub struct SplitLock {
    state: AtomicUsize,
}

impl SplitLock {
    /// create a new splittable lock
    pub const fn new() -> Self {
        SplitLock {
            state: AtomicUsize::new(0),
        }
    }

    /// create a new splittable raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a new splittable mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl SplitLock {
    #[inline]
    fn unlock_fast(&self) -> bool {
        let mut state = self.state.load(Ordering::Relaxed);

        while state > INC || state & PARK_BIT != 0 {
            if let Err(x) = self.state.compare_exchange_weak(
                state,
                state - INC,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                return true;
            }
        }

        false
    }

    #[cold]
    #[inline(never)]
    fn lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state < INC {
                match self.state.compare_exchange_weak(
                    state,
                    state | INC,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return true,
                    Err(x) => state = x,
                }
                continue;
            }

            // If there is no queue, try spinning a few times
            if state & PARK_BIT == 0 && spinwait.spin() {
                state = self.state.load(Ordering::Relaxed);
                continue;
            }

            // Set the parked bit
            if state & PARK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | PARK_BIT,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = x;
                    continue;
                }
            }

            // Park our thread until we are woken up by an unlock
            let addr = self as *const _ as usize;
            // check if locked and parked bit is set
            let validate = || self.state.load(Ordering::Relaxed) != 0;
            let before_sleep = || {};
            let timed_out = |_, was_last_thread| {
                // Clear the parked bit if we were the last parked thread
                if was_last_thread {
                    self.state.fetch_and(!PARK_BIT, Ordering::Relaxed);
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
                    self.state.store(INC, Ordering::Relaxed);
                }
                return TOKEN_HANDOFF;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            if result.have_more_threads {
                self.state.store(PARK_BIT, Ordering::Release);
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

impl crate::mutex::RawMutex for SplitLock {}
unsafe impl crate::RawLockInfo for SplitLock {
    const INIT: Self = Self::new();
    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for SplitLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.lock_slow(None);
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        let state = state & PARK_BIT;

        state
            == self
                .state
                .compare_and_swap(state, state | INC, Ordering::Acquire)
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        if !self.unlock_fast() {
            self.unlock_slow(false)
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        if self.state.load(Ordering::Relaxed) == INC | PARK_BIT {
            self.bump_slow(false)
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockFair for SplitLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        if !self.unlock_fast() {
            self.unlock_slow(true)
        }
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) == INC | PARK_BIT {
            self.bump_slow(true)
        }
    }
}

unsafe impl crate::RawTimedLock for SplitLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for SplitLock {
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

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for SplitLock {
    unsafe fn exc_split(&self) {
        self.state.fetch_add(INC, Ordering::Relaxed);
    }
}
