//! an adaptive raw rwlock

use crate::exclusive_lock::RawExclusiveLock;
use crate::share_lock::RawShareLock;

use parking_lot_core::{self, ParkResult, ParkToken, SpinWait, UnparkResult, UnparkToken};

const PARK_BIT: usize = 0b0001;
const EXC_PARK_BIT: usize = 0b0010;
const EXC_BIT: usize = 0b0100;
const INC: usize = 0b1000;
const COUNT: usize = !(PARK_BIT | EXC_PARK_BIT | EXC_BIT);

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF_EXCLUSIVE: UnparkToken = UnparkToken(1);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF_SHARED: UnparkToken = UnparkToken(2);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_EXCLUSIVE: ParkToken = ParkToken(1);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_SHARED: ParkToken = ParkToken(2);

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

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

/// a splittable raw rwlock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
pub type RawRwLock = crate::rwlock::raw::RwLock<SplitLock>;

/// a splittable rwlock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
pub type RwLock<T> = crate::rwlock::RwLock<SplitLock, T>;

/// a splittable lock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
pub struct SplitLock {
    state: AtomicUsize,
}

impl SplitLock {
    /// Create a new adaptive rwlock lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    /// Create a new adaptive raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new adaptive mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// Create a new adaptive raw rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    /// Create a new adaptive rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }
}

impl crate::mutex::RawMutex for SplitLock {}
unsafe impl crate::rwlock::RawRwLock for SplitLock {}
unsafe impl crate::RawLockInfo for SplitLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = ();
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for SplitLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.exc_lock_slow(None);
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);

        state & !PARK_BIT == 0
            && self
                .state
                .compare_exchange(
                    state,
                    state | EXC_BIT | INC,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        if !self.unlock_fast() {
            self.exc_unlock_slow(false);
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        if self.state.load(Ordering::Relaxed) & PARK_BIT != 0 {
            self.exc_bump_slow(false);
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockFair for SplitLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        if !self.unlock_fast() {
            self.exc_unlock_slow(true);
        }
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) & PARK_BIT != 0 {
            self.exc_bump_slow(true);
        }
    }
}

unsafe impl RawShareLock for SplitLock {
    #[inline]
    fn shr_lock(&self) {
        if !self.shr_try_lock() {
            self.shr_lock_slow(None);
        }
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);
        let (next_state, overflow) = state.overflowing_add(INC);

        state & EXC_BIT == 0
            && !overflow
            && self
                .state
                .compare_exchange(state, next_state, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
    }

    #[inline]
    unsafe fn shr_split(&self) {
        self.state.fetch_add(INC, Ordering::Relaxed);
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        if !self.unlock_fast() {
            self.shr_unlock_slow(false);
        }
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        if self.state.load(Ordering::Relaxed) & PARK_BIT != 0 {
            self.shr_bump_slow(false);
        }
    }
}

unsafe impl crate::share_lock::RawShareLockFair for SplitLock {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        if !self.unlock_fast() {
            self.shr_unlock_slow(true);
        }
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        if self.state.load(Ordering::Relaxed) & PARK_BIT != 0 {
            self.shr_bump_slow(true);
        }
    }
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for SplitLock {
    unsafe fn exc_split(&self) {
        self.state.fetch_add(INC, Ordering::Relaxed);
    }
}

impl SplitLock {
    #[cold]
    fn exc_bump_slow(&self, force_fair: bool) {
        self.exc_unlock_slow(force_fair);
        self.exc_lock();
    }

    #[cold]
    fn shr_bump_slow(&self, force_fair: bool) {
        self.shr_unlock_slow(force_fair);
        self.shr_lock();
    }

    fn unlock_fast(&self) -> bool {
        let mut state = self.state.load(Ordering::Relaxed);

        debug_assert!(state >= INC);

        while state & COUNT >= 2 * INC {
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
    fn unpark_shared(&self) {
        use parking_lot_core::FilterOp;
        use std::cell::Cell;

        let count = Cell::new(0);

        let key = self as *const _ as usize;
        let filter = |token| {
            if self
                .state
                .load(Ordering::Relaxed)
                .checked_add(count.get())
                .and_then(|state| state.checked_add(INC))
                .is_none()
            {
                FilterOp::Stop
            } else if token == TOKEN_SHARED {
                count.set(count.get() + INC);
                FilterOp::Unpark
            } else {
                FilterOp::Skip
            }
        };
        let callback = |_| {
            let count = count.get();
            let state = self.state.fetch_add(count, Ordering::Acquire);
            debug_assert!(state.checked_add(count).is_some());
            TOKEN_HANDOFF_SHARED
        };

        unsafe {
            parking_lot_core::unpark_filter(key, filter, callback);
        }
    }

    #[cold]
    #[inline(never)]
    fn exc_unlock_slow(&self, force_fair: bool) {
        let key = self as *const _ as usize;
        let callback = |result: UnparkResult| {
            if result.unparked_threads != 0 && (force_fair || result.be_fair) {
                if result.have_more_threads {
                    self.state.fetch_or(PARK_BIT, Ordering::Release);
                }

                TOKEN_HANDOFF_EXCLUSIVE
            } else {
                if result.have_more_threads {
                    self.state.store(PARK_BIT, Ordering::Release);
                } else {
                    self.state.store(0, Ordering::Release);
                }

                TOKEN_NORMAL
            }
        };

        unsafe {
            parking_lot_core::unpark_one(key, callback);
        }
    }

    #[cold]
    #[inline(never)]
    fn shr_unlock_slow(&self, force_fair: bool) {
        // this is the last reader, but there may be new
        // shared locks acquired during this call

        if self.state.load(Ordering::Relaxed) & EXC_PARK_BIT == 0 {
            let key = self as *const _ as usize;
            let callback = |result: UnparkResult| {
                if result.unparked_threads != 0 && (force_fair || result.be_fair) {
                    if result.have_more_threads {
                        self.state.fetch_or(PARK_BIT, Ordering::Release);
                    }

                    TOKEN_HANDOFF_SHARED
                } else {
                    if result.have_more_threads {
                        self.state.fetch_sub(INC, Ordering::Release);
                    } else {
                        let mut state = self.state.load(Ordering::Relaxed);

                        loop {
                            let new_state = (state - INC) & !PARK_BIT;

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
                    }

                    TOKEN_NORMAL
                }
            };

            unsafe {
                parking_lot_core::unpark_one(key, callback);
            }
        } else {
            self.state.fetch_sub(INC, Ordering::Release);
            let key = self as *const _ as usize + 1;
            let callback = |result: UnparkResult| {
                if result.unparked_threads != 0 {
                    self.state.fetch_and(!EXC_PARK_BIT, Ordering::Relaxed);
                }
                TOKEN_NORMAL
            };

            unsafe {
                parking_lot_core::unpark_one(key, callback);
            }
        }
    }

    #[cold]
    fn wait_for_shared(&self, timeout: Option<Instant>) -> bool {
        let mut state = self.state.fetch_or(EXC_BIT, Ordering::Acquire);
        let mut wait = SpinWait::new();

        while state & COUNT != 0 {
            if wait.spin() {
                state = self.state.load(Ordering::Relaxed);
                continue;
            }

            // Set the parked bit
            if state & EXC_PARK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | EXC_PARK_BIT,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = x;
                    continue;
                }
            }

            // Park our thread until we are woken up by an unlock
            // Using the 2nd key at addr + 1
            let addr = self as *const _ as usize + 1;
            let validate = || {
                let state = self.state.load(Ordering::Relaxed);
                state & COUNT != 0 && state & EXC_PARK_BIT != 0
            };
            let before_sleep = || {};
            let timed_out = |_, _| {};

            // SAFETY:
            //   * `addr` is an address we control.
            //   * `validate`/`timed_out` does not panic or call into any function of `parking_lot`.
            //   * `before_sleep` does not call `park`, nor does it panic.
            let park_result = unsafe {
                parking_lot_core::park(
                    addr,
                    validate,
                    before_sleep,
                    timed_out,
                    TOKEN_EXCLUSIVE,
                    timeout,
                )
            };

            match park_result {
                // We still need to re-check the state if we are unparked
                // since a previous writer timing-out could have allowed
                // another reader to sneak in before we parked.
                ParkResult::Unparked(_) | ParkResult::Invalid => {
                    state = self.state.load(Ordering::Relaxed);
                    continue;
                }

                // Timeout expired
                ParkResult::TimedOut => {
                    self.state
                        .fetch_and(!(EXC_BIT | EXC_PARK_BIT), Ordering::Relaxed);

                    self.unpark_shared();

                    return false;
                }
            }
        }

        self.state.fetch_or(INC, Ordering::Relaxed);

        true
    }

    #[cold]
    #[inline(never)]
    fn exc_lock_slow(&self, timeout: Option<Instant>) -> bool {
        let try_lock = |state: &mut usize| loop {
            if *state & COUNT != 0 {
                return false;
            }

            // Grab EXC_BIT if it isn't set, even if there are parked threads.
            match self.state.compare_exchange_weak(
                *state,
                *state | EXC_BIT | INC,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(x) => *state = x,
            }
        };

        let exclusive = || true;
        let shared = || {
            if self.state.load(Ordering::Relaxed) & PARK_BIT != 0 {
                self.wait_for_shared(timeout)
            } else {
                true
            }
        };

        self.lock_slow(
            TOKEN_EXCLUSIVE,
            timeout,
            EXC_BIT,
            try_lock,
            exclusive,
            shared,
        )
    }

    #[cold]
    #[inline(never)]
    fn shr_lock_slow(&self, timeout: Option<Instant>) -> bool {
        let try_lock = |state: &mut usize| {
            let mut wait = SpinWait::new();

            loop {
                if *state & EXC_BIT != 0 {
                    return false;
                }

                if self
                    .state
                    .compare_exchange_weak(
                        *state,
                        state
                            .checked_add(INC)
                            .expect("RwLock reader count overflow"),
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return true;
                }

                wait.spin();
                *state = self.state.load(Ordering::Relaxed);
            }
        };

        let exclusive = || {
            let mut state = self.state.load(Ordering::Relaxed);

            while let Err(x) = self.state.compare_exchange_weak(
                state,
                (state & PARK_BIT) | INC,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                state = x;
            }

            if state & PARK_BIT != 0 {
                self.unpark_shared();
            }

            true
        };
        let shared = || true;

        self.lock_slow(TOKEN_SHARED, timeout, EXC_BIT, try_lock, exclusive, shared)
    }

    #[inline]
    fn lock_slow(
        &self,
        park_token: ParkToken,
        timeout: Option<Instant>,
        validate_flags: usize,
        mut try_lock: impl FnMut(&mut usize) -> bool,
        exclusive: impl FnOnce() -> bool,
        shared: impl FnOnce() -> bool,
    ) -> bool {
        let mut wait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            if try_lock(&mut state) {
                return true;
            }

            // If there are no parked threads, try spinning a few times.
            if state & (PARK_BIT | EXC_PARK_BIT) == 0 && wait.spin() {
                state = self.state.load(Ordering::Relaxed);
                continue;
            }

            // Set the park bit
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
            let validate = || {
                let state = self.state.load(Ordering::Relaxed);
                state & PARK_BIT != 0 && (state & validate_flags != 0)
            };
            let before_sleep = || {};
            let timed_out = |_, was_last_thread| {
                // Clear the parked bit if we were the last parked thread
                if was_last_thread {
                    self.state.fetch_and(!PARK_BIT, Ordering::Relaxed);
                }
            };

            // SAFETY:
            // * `addr` is an address we control.
            // * `validate`/`timed_out` does not panic or call into any function of `parking_lot`.
            // * `before_sleep` does not call `park`, nor does it panic.
            let park_result = unsafe {
                parking_lot_core::park(addr, validate, before_sleep, timed_out, park_token, timeout)
            };

            match park_result {
                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(TOKEN_HANDOFF_EXCLUSIVE) => {
                    return exclusive();
                }

                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(TOKEN_HANDOFF_SHARED) => {
                    return shared();
                }

                // We were unparked normally, try acquiring the lock again
                ParkResult::Unparked(_) => (),

                // The validation function failed, try locking again
                ParkResult::Invalid => (),

                // Timeout expired
                ParkResult::TimedOut => return false,
            }

            // Loop back and try locking again
            wait.reset();
            state = self.state.load(Ordering::Relaxed);
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for SplitLock {
    type Instant = Instant;
    type Duration = Duration;

    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        if self.exc_try_lock() {
            true
        } else {
            self.exc_lock_slow(Some(instant))
        }
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        if self.exc_try_lock() {
            true
        } else {
            self.exc_lock_slow(Instant::now().checked_add(duration))
        }
    }
}

unsafe impl crate::share_lock::RawShareLockTimed for SplitLock {
    type Instant = Instant;
    type Duration = Duration;

    fn shr_try_lock_until(&self, instant: Self::Instant) -> bool {
        if self.shr_try_lock() {
            true
        } else {
            self.shr_lock_slow(Some(instant))
        }
    }

    fn shr_try_lock_for(&self, duration: Self::Duration) -> bool {
        if self.shr_try_lock() {
            true
        } else {
            self.shr_lock_slow(Instant::now().checked_add(duration))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_utils::sync::WaitGroup;

    #[test]
    fn wait_for_shared() {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        static LOCK: RawRwLock = SplitLock::raw_rwlock();

        let wait = WaitGroup::new();

        LOCK.inner().state.store(0, Ordering::Release);
        SEQUENCE.store(0, Ordering::Release);

        let lock = LOCK.read();

        let t = std::thread::spawn({
            let wait = wait.clone();
            move || {
                assert_eq!(SEQUENCE.load(Ordering::Relaxed), 0);
                wait.wait();
                LOCK.inner().wait_for_shared(None);
                assert_eq!(SEQUENCE.fetch_add(1, Ordering::Relaxed), 1);
            }
        });

        wait.wait();

        assert_eq!(SEQUENCE.fetch_add(1, Ordering::Relaxed), 0);
        drop(lock);
        t.join().unwrap();
        assert_eq!(SEQUENCE.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn shared_to_exclusive() {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        static LOCK: RawRwLock = SplitLock::raw_rwlock();

        let wait = WaitGroup::new();

        let lock = LOCK.read();

        let t = std::thread::spawn({
            let wait = wait.clone();
            move || {
                assert_eq!(SEQUENCE.load(Ordering::Relaxed), 0);
                wait.wait();
                let lock = LOCK.write();
                assert_eq!(SEQUENCE.fetch_add(1, Ordering::Relaxed), 1);

                let boo = lock.clone();

                drop(lock);
                drop(boo);
            }
        });

        wait.wait();

        assert_eq!(SEQUENCE.fetch_add(1, Ordering::Relaxed), 0);
        lock.unlock_fair();
        t.join().unwrap();
        assert_eq!(SEQUENCE.load(Ordering::Relaxed), 2);

        assert!(LOCK.try_read().is_some());
    }
}
