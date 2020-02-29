use parking_lot_core::{
    self, ParkResult, RequeueOp, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN,
};

use super::{Parkable, WaitTimeoutResult};
use crate::exclusive_lock::{raw::RawExclusiveGuard, RawExclusiveLock};
use crate::share_lock::{raw::RawShareGuard, RawShareLock};
use crate::RawLockInfo;

use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::{Duration, Instant};

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
pub const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
pub const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

pub struct RawCondvar<L> {
    lock: AtomicPtr<L>,
}

unsafe impl<L: Parkable + RawLockInfo + Sync> Send for RawCondvar<L> {}
unsafe impl<L: Parkable + RawLockInfo + Sync> Sync for RawCondvar<L> {}

impl<L> RawCondvar<L> {
    pub const fn new() -> Self {
        Self {
            lock: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl<L: Parkable> RawCondvar<L> {
    #[inline]
    pub fn notify_one(&self) -> bool {
        let lock = self.lock.load(Ordering::Relaxed);

        if lock.is_null() {
            false
        } else {
            self.notify_one_slow(lock)
        }
    }

    #[cold]
    fn notify_one_slow(&self, lock: *mut L) -> bool {
        unsafe {
            // Unpark one thread and requeue the rest onto the mutex
            let from = self as *const _ as usize;
            let to = lock as usize;
            let validate = || {
                // Make sure that our atomic state still points to the same
                // mutex. If not then it means that all threads on the current
                // mutex were woken up and a new waiting thread switched to a
                // different mutex. In that case we can get away with doing
                // nothing.
                if self.lock.load(Ordering::Relaxed) != lock {
                    return RequeueOp::Abort;
                }

                // Unpark one thread if the mutex is unlocked, otherwise just
                // requeue everything to the mutex. This is safe to do here
                // since unlocking the mutex when the parked bit is set requires
                // locking the queue. There is the possibility of a race if the
                // mutex gets locked after we check, but that doesn't matter in
                // this case.
                if (*lock).mark_parked_if_locked() {
                    RequeueOp::RequeueOne
                } else {
                    RequeueOp::UnparkOne
                }
            };
            let callback = |_op, result: UnparkResult| {
                // Clear our state if there are no more waiting threads
                if !result.have_more_threads {
                    self.lock.store(ptr::null_mut(), Ordering::Relaxed);
                }

                TOKEN_NORMAL
            };
            let res = parking_lot_core::unpark_requeue(from, to, validate, callback);

            res.unparked_threads + res.requeued_threads != 0
        }
    }

    #[inline]
    pub fn notify_all(&self) -> usize {
        // Nothing to do if there are no waiting threads
        let lock = self.lock.load(Ordering::Relaxed);
        if lock.is_null() {
            return 0;
        }

        self.notify_all_slow(lock)
    }

    #[cold]
    fn notify_all_slow(&self, lock: *mut L) -> usize {
        unsafe {
            // Unpark one thread and requeue the rest onto the mutex
            let from = self as *const _ as usize;
            let to = lock as usize;
            let validate = || {
                // Make sure that our atomic state still points to the same
                // mutex. If not then it means that all threads on the current
                // mutex were woken up and a new waiting thread switched to a
                // different mutex. In that case we can get away with doing
                // nothing.
                if self.lock.load(Ordering::Relaxed) != lock {
                    return RequeueOp::Abort;
                }

                // Clear our state since we are going to unpark or requeue all
                // threads.
                self.lock.store(ptr::null_mut(), Ordering::Relaxed);

                // Unpark one thread if the mutex is unlocked, otherwise just
                // requeue everything to the mutex. This is safe to do here
                // since unlocking the mutex when the parked bit is set requires
                // locking the queue. There is the possibility of a race if the
                // mutex gets locked after we check, but that doesn't matter in
                // this case.
                if (*lock).mark_parked_if_locked() {
                    RequeueOp::RequeueAll
                } else {
                    RequeueOp::UnparkOneRequeueRest
                }
            };
            let callback = |op, result: UnparkResult| {
                // If we requeued threads to the mutex, mark it as having
                // parked threads. The RequeueAll case is already handled above.
                if op == RequeueOp::UnparkOneRequeueRest && result.requeued_threads != 0 {
                    (*lock).mark_parked();
                }

                TOKEN_NORMAL
            };
            let res = parking_lot_core::unpark_requeue(from, to, validate, callback);

            res.unparked_threads + res.requeued_threads
        }
    }

    #[cold]
    #[inline(never)]
    unsafe fn wait_until_internal(
        &self,
        lock_addr: *mut L,
        timeout: Option<Instant>,
        lock: &dyn Fn(),
        unlock: &dyn Fn(),
    ) -> WaitTimeoutResult {
        let result;
        let mut bad_mutex = false;
        let mut requeued = false;
        {
            let addr = self as *const _ as usize;
            let validate = || {
                // Ensure we don't use two different mutexes with the same
                // Condvar at the same time. This is done while locked to
                // avoid races with notify_one
                let lock = self.lock.load(Ordering::Relaxed);
                if lock.is_null() {
                    self.lock.store(lock_addr, Ordering::Relaxed);
                } else if lock != lock_addr {
                    bad_mutex = true;
                    return false;
                }
                true
            };
            let before_sleep = unlock;
            let timed_out = |k, was_last_thread| {
                // If we were requeued to a mutex, then we did not time out.
                // We'll just park ourselves on the mutex again when we try
                // to lock it later.
                requeued = k != addr;

                // If we were the last thread on the queue then we need to
                // clear our state. This is normally done by the
                // notify_{one,all} functions when not timing out.
                if !requeued && was_last_thread {
                    self.lock.store(ptr::null_mut(), Ordering::Relaxed);
                }
            };

            result = parking_lot_core::park(
                addr,
                validate,
                before_sleep,
                timed_out,
                DEFAULT_PARK_TOKEN,
                timeout,
            );
        }

        // Panic if we tried to use multiple mutexes with a Condvar. Note
        // that at this point the MutexGuard is still locked. It will be
        // unlocked by the unwinding logic.
        if bad_mutex {
            panic!("attempted to use a condition variable with more than one lock");
        }

        // ... and re-lock it once we are done sleeping
        if result != ParkResult::Unparked(TOKEN_HANDOFF) {
            lock()
        }

        WaitTimeoutResult(!(result.is_unparked() || requeued))
    }
}

impl<L: RawExclusiveLock + RawLockInfo + Parkable> RawCondvar<L> {
    #[inline]
    fn exc_wait_until_internal(&self, lock: &L, timeout: Option<Instant>) -> WaitTimeoutResult {
        unsafe {
            self.wait_until_internal(
                lock as *const L as *mut L,
                timeout,
                &|| lock.exc_lock(),
                &|| lock.exc_unlock(),
            )
        }
    }

    #[inline]
    pub fn exc_wait(&self, guard: &mut RawExclusiveGuard<L>) {
        self.exc_wait_until_internal(guard.inner(), None);
    }

    #[inline]
    pub fn exc_wait_until(
        &self,
        guard: &mut RawExclusiveGuard<L>,
        instant: Instant,
    ) -> WaitTimeoutResult {
        self.exc_wait_until_internal(guard.inner(), Some(instant))
    }

    #[inline]
    pub fn exc_wait_for(
        &self,
        guard: &mut RawExclusiveGuard<L>,
        duration: Duration,
    ) -> WaitTimeoutResult {
        self.exc_wait_until_internal(guard.inner(), Instant::now().checked_add(duration))
    }
}

impl<L: RawShareLock + RawLockInfo + Parkable> RawCondvar<L> {
    #[inline]
    fn shr_wait_until_internal(&self, lock: &L, timeout: Option<Instant>) -> WaitTimeoutResult {
        unsafe {
            self.wait_until_internal(
                lock as *const L as *mut L,
                timeout,
                &|| lock.shr_lock(),
                &|| lock.shr_unlock(),
            )
        }
    }

    #[inline]
    pub fn shr_wait(&self, guard: &mut RawShareGuard<L>) {
        self.shr_wait_until_internal(guard.inner(), None);
    }

    #[inline]
    pub fn shr_wait_until(
        &self,
        guard: &mut RawShareGuard<L>,
        instant: Instant,
    ) -> WaitTimeoutResult {
        self.shr_wait_until_internal(guard.inner(), Some(instant))
    }

    #[inline]
    pub fn shr_wait_for(
        &self,
        guard: &mut RawShareGuard<L>,
        duration: Duration,
    ) -> WaitTimeoutResult {
        self.shr_wait_until_internal(guard.inner(), Instant::now().checked_add(duration))
    }
}
