use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockDowngrade};
use crate::share_lock::RawShareLock;

use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF_EXCLUSIVE: UnparkToken = UnparkToken(1);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF_SHARED: UnparkToken = UnparkToken(2);

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

pub struct RawLock {
    state: AtomicUsize,
}

impl RawLock {
    const PARK_BIT: usize = 1;
    const INC: usize = 2;
    const UNIQ_LOCK: usize = usize::max_value() & !Self::PARK_BIT;
    const LOCK_MASK: usize = usize::max_value() & !Self::PARK_BIT;

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }

    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        unsafe { RwLock::from_raw_parts(Self::new(), value) }
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::rwlock::RawRwLock for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = ();
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        if !self.uniq_try_lock() {
            self.uniq_lock_slow(None);
        }
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        self.state
            .compare_exchange(0, Self::UNIQ_LOCK, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        if self
            .state
            .compare_exchange(Self::UNIQ_LOCK, 0, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            self.uniq_unlock_slow(false);
        }
    }

    #[inline]
    unsafe fn uniq_bump(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.uniq_bump_slow(false);
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockFair for RawLock {
    #[inline]
    unsafe fn uniq_unlock_fair(&self) {
        if self
            .state
            .compare_exchange(Self::UNIQ_LOCK, 0, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            self.uniq_unlock_slow(true);
        }
    }

    #[inline]
    unsafe fn uniq_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.uniq_bump_slow(true);
        }
    }
}

unsafe impl RawShareLock for RawLock {
    #[inline]
    fn shr_lock(&self) {
        if !self.shr_try_lock() {
            self.shr_lock_slow(None);
        }
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);
        let (next_state, overflow) = state.overflowing_add(Self::INC);

        !overflow
            && self
                .state
                .compare_exchange(state, next_state, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
    }

    #[inline]
    unsafe fn shr_split(&self) {
        let was_locked = self.shr_try_lock();
        assert!(was_locked, "Tried to create too many shared locks!");
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.shr_unlock_inner(false)
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.shr_bump_slow(false);
        }
    }
}

unsafe impl crate::share_lock::RawShareLockFair for RawLock {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        self.shr_unlock_inner(true)
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) & Self::PARK_BIT != 0 {
            self.shr_bump_slow(true);
        }
    }
}

unsafe impl RawExclusiveLockDowngrade for RawLock {
    unsafe fn downgrade(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        while let Err(x) = self.state.compare_exchange_weak(
            state,
            (state & Self::PARK_BIT) | Self::INC,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            state = x;
        }
    }
}

impl RawLock {
    #[inline]
    fn shr_unlock_inner(&self, force_fair: bool) {
        let mut state = self.state.load(Ordering::Relaxed);

        while state & Self::PARK_BIT == 0 {
            if let Err(x) = self.state.compare_exchange(
                state,
                state - Self::INC,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                return;
            }
        }

        self.shr_unlock_slow(force_fair);
    }

    #[cold]
    #[inline(never)]
    fn uniq_lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state & Self::LOCK_MASK == 0 {
                match self.state.compare_exchange_weak(
                    state,
                    state | Self::UNIQ_LOCK,
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
                let state = self.state.load(Ordering::Relaxed);
                state & Self::LOCK_MASK != 0 && state & Self::PARK_BIT != 0
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
                ParkResult::Unparked(self::TOKEN_HANDOFF_EXCLUSIVE) => return true,

                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(self::TOKEN_HANDOFF_SHARED) => {
                    let mut state = self.state.load(Ordering::Relaxed);

                    while state & Self::LOCK_MASK == Self::INC {
                        if let Err(x) = self.state.compare_exchange_weak(
                            state,
                            Self::LOCK_MASK | state,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        ) {
                            state = x
                        } else {
                            return true;
                        }
                    }
                }

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
    fn shr_lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if let Some(next_state) = (state & Self::LOCK_MASK).checked_add(Self::INC) {
                match self.state.compare_exchange_weak(
                    state,
                    next_state,
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
                let state = self.state.load(Ordering::Relaxed);
                state & Self::LOCK_MASK != Self::LOCK_MASK && state & Self::PARK_BIT != 0
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
                ParkResult::Unparked(self::TOKEN_HANDOFF_EXCLUSIVE) => {
                    unsafe {
                        self.downgrade();
                    }
                    return true;
                }

                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(self::TOKEN_HANDOFF_SHARED) => return true,

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
    fn uniq_unlock_slow(&self, force_fair: bool) {
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
                return TOKEN_HANDOFF_EXCLUSIVE;
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
    #[inline(never)]
    fn shr_unlock_slow(&self, force_fair: bool) {
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
                return TOKEN_HANDOFF_SHARED;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            let mut state = self.state.load(Ordering::Relaxed);

            loop {
                let mut new_state = state - Self::INC;

                if result.have_more_threads {
                    new_state |= Self::PARK_BIT;
                }

                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    new_state,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    break;
                }
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
    fn uniq_bump_slow(&self, force_fair: bool) {
        self.uniq_unlock_slow(force_fair);
        self.uniq_lock();
    }

    #[cold]
    fn shr_bump_slow(&self, force_fair: bool) {
        self.shr_unlock_slow(force_fair);
        self.shr_lock();
    }
}
