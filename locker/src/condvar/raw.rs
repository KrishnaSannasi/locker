use parking_lot_core::{self, UnparkResult, DEFAULT_PARK_TOKEN, DEFAULT_UNPARK_TOKEN};

use super::{Parkable, WaitTimeoutResult};
use crate::exclusive_lock::{RawExclusiveGuard, RawExclusiveLock};
use crate::share_lock::{RawShareGuard, RawShareLock};
use crate::RawLockInfo;

use core::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct Condvar {
    is_parked: AtomicBool,
}

impl crate::Init for Condvar {
    const INIT: Self = Self {
        is_parked: AtomicBool::new(false),
    };
}

impl Condvar {
    pub const fn new() -> Self {
        Self {
            is_parked: AtomicBool::new(false),
        }
    }
}

impl Condvar {
    #[inline]
    pub fn notify_one(&self) -> bool {
        let is_parked = self.is_parked.load(Ordering::Relaxed);

        if !is_parked {
            false
        } else {
            self.notify_one_slow()
        }
    }

    #[cold]
    fn notify_one_slow(&self) -> bool {
        unsafe {
            // Unpark one thread and requeue the rest onto the mutex
            let key = self as *const _ as usize;
            let callback = |result: UnparkResult| {
                // Clear our state if there are no more waiting threads
                if !result.have_more_threads {
                    self.is_parked.store(false, Ordering::Relaxed);
                }

                DEFAULT_UNPARK_TOKEN
            };
            let res = parking_lot_core::unpark_one(key, callback);

            res.unparked_threads != 0
        }
    }

    #[inline]
    pub fn notify_all(&self) -> usize {
        // Nothing to do if there are no waiting threads
        let is_parked = self.is_parked.load(Ordering::Relaxed);

        if !is_parked {
            return 0;
        }

        self.notify_all_slow()
    }

    #[cold]
    fn notify_all_slow(&self) -> usize {
        unsafe {
            // Unpark one thread and requeue the rest onto the mutex
            let key = self as *const _ as usize;
            let unpark_count = parking_lot_core::unpark_all(key, DEFAULT_UNPARK_TOKEN);
            self.is_parked.store(false, Ordering::Relaxed);
            unpark_count
        }
    }

    #[cold]
    #[inline(never)]
    unsafe fn wait(
        &self,
        timeout: Option<Instant>,
        lock: impl FnOnce(),
        unlock: impl FnOnce(),
    ) -> WaitTimeoutResult {
        let result;
        {
            let addr = self as *const _ as usize;
            let validate = || self.is_parked.load(Ordering::Relaxed);
            let timed_out = |_, was_last_thread| {
                // If we were the last thread on the queue then we need to
                // clear our state. This is normally done by the
                // notify_{one,all} functions when not timing out.
                if was_last_thread {
                    self.is_parked.store(false, Ordering::Relaxed);
                }
            };

            self.is_parked.store(true, Ordering::Relaxed);

            result = parking_lot_core::park(
                addr,
                validate,
                unlock,
                timed_out,
                DEFAULT_PARK_TOKEN,
                timeout,
            );
        }

        lock();

        WaitTimeoutResult(!result.is_unparked())
    }
}

impl Condvar {
    #[inline]
    fn exc_wait_until_internal(
        &self,
        lock: &dyn RawExclusiveLock,
        timeout: Option<Instant>,
    ) -> WaitTimeoutResult {
        unsafe { self.wait(timeout, || lock.exc_lock(), || lock.exc_unlock()) }
    }

    #[inline]
    pub fn exc_wait<L: RawExclusiveLock + RawLockInfo + Parkable>(
        &self,
        guard: &mut RawExclusiveGuard<L>,
    ) {
        self.exc_wait_until_internal(guard.inner(), None);
    }

    #[inline]
    pub fn exc_wait_until<L: RawExclusiveLock + RawLockInfo + Parkable>(
        &self,
        guard: &mut RawExclusiveGuard<L>,
        instant: Instant,
    ) -> WaitTimeoutResult {
        self.exc_wait_until_internal(guard.inner(), Some(instant))
    }

    #[inline]
    pub fn exc_wait_for<L: RawExclusiveLock + RawLockInfo + Parkable>(
        &self,
        guard: &mut RawExclusiveGuard<L>,
        duration: Duration,
    ) -> WaitTimeoutResult {
        self.exc_wait_until_internal(guard.inner(), Instant::now().checked_add(duration))
    }
}

impl Condvar {
    #[inline]
    fn shr_wait_until_internal(
        &self,
        lock: &dyn RawShareLock,
        timeout: Option<Instant>,
    ) -> WaitTimeoutResult {
        unsafe { self.wait(timeout, || lock.shr_lock(), || lock.shr_unlock()) }
    }

    #[inline]
    pub fn shr_wait<L: RawShareLock + RawLockInfo + Parkable>(&self, guard: &mut RawShareGuard<L>) {
        self.shr_wait_until_internal(guard.inner(), None);
    }

    #[inline]
    pub fn shr_wait_until<L: RawShareLock + RawLockInfo + Parkable>(
        &self,
        guard: &mut RawShareGuard<L>,
        instant: Instant,
    ) -> WaitTimeoutResult {
        self.shr_wait_until_internal(guard.inner(), Some(instant))
    }

    #[inline]
    pub fn shr_wait_for<L: RawShareLock + RawLockInfo + Parkable>(
        &self,
        guard: &mut RawShareGuard<L>,
        duration: Duration,
    ) -> WaitTimeoutResult {
        self.shr_wait_until_internal(guard.inner(), Instant::now().checked_add(duration))
    }
}
