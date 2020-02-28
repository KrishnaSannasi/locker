use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF_UNIQUE: UnparkToken = UnparkToken(1);
// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF_SHARED: UnparkToken = UnparkToken(2);

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

const PARK_BIT: usize = 0b001;
const LOCK_BIT: usize = 0b010;
const UNIQ_BIT: usize = 0b100;

const MASK: usize = INC - 1;
const INC: usize = 0b1000;

pub struct RawLock {
    state: AtomicUsize,
}

impl RawLock {
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

    #[cold]
    #[inline(never)]
    fn uniq_lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let key = self as *const _ as usize;

        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    0,
                    LOCK_BIT | UNIQ_BIT | INC,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    return true;
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
            let validate = || {
                let state = self.state.load(Ordering::Relaxed);

                state & (LOCK_BIT | PARK_BIT) != 0
            };

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
                    key,
                    validate,
                    before_sleep,
                    timed_out,
                    DEFAULT_PARK_TOKEN,
                    timeout,
                )
            } {
                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(self::TOKEN_HANDOFF_UNIQUE) => return true,

                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(self::TOKEN_HANDOFF_SHARED) => {
                    // this while loop is necessary because while the `UNIQ_BIT` is not set
                    // a shr lock could be acquired, this includes the time between sending the
                    // `TOKEN_HANDOFF_SHARED` and setting the `UNIQ_BIT`.
                    //
                    // i.e. there is a race condition that must be taken care of

                    let mut state = self.state.load(Ordering::Acquire);

                    while state < INC * 2 {
                        // if that was the last shared lock
                        // then we can change it to a unique lock
                        // without having to reaquire the lock

                        if let Err(x) = self.state.compare_exchange_weak(
                            state,
                            state | UNIQ_BIT,
                            Ordering::Relaxed,
                            Ordering::Relaxed,
                        ) {
                            state = x;
                        } else {
                            // if we successfully set the `UNIQ_BIT`
                            return true;
                        }
                    }

                    // if we failed to acquire a unique lock before another shared lock
                    // acquired the lock
                    self.state.fetch_sub(INC, Ordering::Release);
                }

                // We were unparked normally, try acquiring the lock again
                // or we got a handoff token from a shared unlock
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
                    self.state.fetch_and(!PARK_BIT, Ordering::Relaxed);
                }

                return TOKEN_HANDOFF_UNIQUE;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            // Also clear the counter becase this is the last lock
            if result.have_more_threads {
                self.state
                    .fetch_and(!(LOCK_BIT | UNIQ_BIT) & MASK, Ordering::Release);
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
    fn shr_lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let key = self as *const _ as usize;

        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    0,
                    LOCK_BIT | INC,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    return true;
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
            let validate = || {
                let state = self.state.load(Ordering::Relaxed);

                state & (LOCK_BIT | PARK_BIT) != 0
            };

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
                    key,
                    validate,
                    before_sleep,
                    timed_out,
                    DEFAULT_PARK_TOKEN,
                    timeout,
                )
            } {
                // The thread that unparked us passed the lock on to us
                // directly without unlocking it.
                ParkResult::Unparked(self::TOKEN_HANDOFF_UNIQUE) => {
                    // remove the uniq bit, because we are now a shared lock
                    self.state.fetch_and(!UNIQ_BIT, Ordering::Relaxed);
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
                    self.state.fetch_and(!PARK_BIT, Ordering::Relaxed);
                }

                return TOKEN_HANDOFF_SHARED;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            // Also clear the counter becase this is the last lock
            if result.have_more_threads {
                self.state.fetch_and(!LOCK_BIT & MASK, Ordering::Release);
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

    fn split(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            let new_state = state
                .checked_add(INC)
                .expect("tried to split too many times");

            if let Err(x) = self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                return;
            }
        }
    }
}

unsafe impl crate::RawLockInfo for RawLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type UniqueGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::unique_lock::RawUniqueLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        if !self.uniq_try_lock() {
            self.uniq_lock_slow(None);
        }
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        self.state
            .compare_exchange(
                0,
                LOCK_BIT | UNIQ_BIT | INC,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        // if this is the final lock, and there are no parked threads
        if self
            .state
            .compare_exchange(
                LOCK_BIT | UNIQ_BIT | INC,
                0,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            return;
        }

        let mut state = self.state.load(Ordering::Acquire);

        // if not final lock
        while state >= INC * 2 {
            if let Err(x) = self.state.compare_exchange_weak(
                state,
                state - INC,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                // if we were able to decrement, then we released our lock
                return;
            }
        }

        // if there are parked threads
        self.uniq_unlock_slow(false);
    }
}

unsafe impl crate::unique_lock::SplittableUniqueLock for RawLock {
    unsafe fn uniq_split(&self) {
        self.split()
    }
}

unsafe impl crate::share_lock::RawShareLock for RawLock {
    #[inline]
    fn shr_lock(&self) {
        if !self.shr_try_lock() {
            self.shr_lock_slow(None);
        }
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);

        if state & UNIQ_BIT != 0 {
            // if there is a uniq lock, we can't acquire a lock
            false
        } else if let Some(new_state) = state.checked_add(INC) {
            self.state
                .compare_exchange(
                    state,
                    new_state | LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
        } else {
            false
        }
    }

    #[inline]
    unsafe fn shr_split(&self) {
        self.split()
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        // if this is the final lock, and there are no parked threads
        if self
            .state
            .compare_exchange(LOCK_BIT | INC, 0, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        let mut state = self.state.load(Ordering::Acquire);

        // if not final lock
        while state >= INC * 2 {
            if let Err(x) = self.state.compare_exchange_weak(
                state,
                state - INC,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                // if we were able to decrement, then we released our lock
                return;
            }
        }

        // if there are parked threads
        self.shr_unlock_slow(false);
    }
}

#[test]
fn test_writes() {
    let m = RawLock::mutex(10);

    let _lock = m.lock();
    assert!(m.try_lock().is_none());
    drop(_lock);

    let m = std::sync::Arc::new(m);

    let t = std::thread::spawn({
        let m = m.clone();

        move || {
            std::thread::sleep(std::time::Duration::from_micros(100));

            assert!(m.try_lock().is_none());

            let _lock = m.lock();

            assert_eq!(*_lock, 100);
        }
    });

    let mut _lock = m.lock();

    *_lock = 100;

    std::thread::sleep(std::time::Duration::from_millis(100));

    drop(_lock);

    t.join().unwrap();
}

#[test]
fn test_reads() {
    let m = RawLock::rwlock(10);

    {
        let _a = m.read();
        let _b = m.read();
        let _c = crate::share_lock::ShareGuard::clone(&_b);
    }

    {
        let _a = m.try_write().unwrap();
    }
}
