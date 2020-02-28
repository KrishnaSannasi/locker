use parking_lot_core::{
    self, ParkResult, RequeueOp, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN,
};

use crate::exclusive_lock::{RawExclusiveLock, ExclusiveGuard};
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

pub struct Condvar<L> {
    lock: AtomicPtr<L>,
}

/// # Safety
///
/// `uniq_unlock` cannot call `parking_lot_core::park`, or panic
pub unsafe trait Parkable: RawExclusiveLock {
    fn mark_parked_if_locked(&self) -> bool;
    fn mark_parked(&self);
}

/// A type indicating whether a timed wait on a condition variable returned
/// due to a time out or not.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct WaitTimeoutResult(bool);

impl WaitTimeoutResult {
    /// Returns whether the wait was known to have timed out.
    #[inline]
    pub fn timed_out(self) -> bool {
        self.0
    }
}

impl<L> Condvar<L> {
    pub const fn new() -> Self {
        Self {
            lock: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl<L: RawExclusiveLock + RawLockInfo + Parkable> Condvar<L> {
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

    // This is a non-generic function to reduce the monomorphization cost of
    // using `wait_until`.
    #[cold]
    #[inline(never)]
    fn wait_until_internal(&self, lock: &L, timeout: Option<Instant>) -> WaitTimeoutResult {
        unsafe {
            let result;
            let mut bad_mutex = false;
            let mut requeued = false;
            {
                let addr = self as *const _ as usize;
                let lock_addr = lock as *const _ as *mut _;
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
                let before_sleep = || {
                    // Unlock the mutex before sleeping...
                    lock.uniq_unlock();
                };
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
                panic!("attempted to use a condition variable with more than one mutex");
            }

            // ... and re-lock it once we are done sleeping
            if result != ParkResult::Unparked(TOKEN_HANDOFF) {
                lock.uniq_lock();
            }

            WaitTimeoutResult(!(result.is_unparked() || requeued))
        }
    }

    pub fn wait<T: ?Sized>(&self, guard: &mut ExclusiveGuard<L, T>) {
        self.wait_until_internal(unsafe { guard.raw().inner() }, None);
    }

    pub fn wait_until<T: ?Sized>(&self, guard: &mut ExclusiveGuard<L, T>, instant: Instant) {
        self.wait_until_internal(unsafe { guard.raw().inner() }, Some(instant));
    }

    pub fn wait_for<T: ?Sized>(&self, guard: &mut ExclusiveGuard<L, T>, duration: Duration) {
        self.wait_until_internal(
            unsafe { guard.raw().inner() },
            Instant::now().checked_add(duration),
        );
    }
}
