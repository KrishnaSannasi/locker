use crate::exclusive_lock::{ExclusiveGuard, RawExclusiveLock};
use crate::share_lock::{RawShareLock, ShareGuard};

use crate::RawLockInfo;

use std::time::{Duration, Instant};

mod raw;
pub use raw::RawCondvar;

pub struct Condvar {
    raw: RawCondvar,
}

/// # Safety
///
/// `exc_unlock` cannot call `parking_lot_core::park`, or panic
pub unsafe trait Parkable {}

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

impl Condvar {
    pub const fn new() -> Self {
        Self {
            raw: RawCondvar::new(),
        }
    }
}

impl Condvar {
    #[inline]
    pub fn notify_one(&self) -> bool {
        self.raw.notify_one()
    }

    #[inline]
    pub fn notify_all(&self) -> usize {
        self.raw.notify_all()
    }

    #[inline]
    pub fn wait<W: Wait + ?Sized>(&self, guard: &mut W) {
        guard.wait(self)
    }

    #[inline]
    pub fn wait_until<W: Wait + ?Sized>(
        &self,
        guard: &mut W,
        instant: Instant,
    ) -> WaitTimeoutResult {
        guard.wait_until(self, instant)
    }

    #[inline]
    pub fn wait_for<W: Wait + ?Sized>(
        &self,
        guard: &mut W,
        duration: Duration,
    ) -> WaitTimeoutResult {
        guard.wait_for(self, duration)
    }
}

pub trait Wait {
    fn wait(&mut self, cv: &Condvar);

    fn wait_until(&mut self, cv: &Condvar, timeout: Instant) -> WaitTimeoutResult;

    fn wait_for(&mut self, cv: &Condvar, duration: Duration) -> WaitTimeoutResult;
}

impl<L: RawLockInfo + RawExclusiveLock + Parkable, T: ?Sized> Wait for ExclusiveGuard<'_, L, T> {
    #[inline]
    fn wait(&mut self, cv: &Condvar) {
        unsafe { cv.raw.exc_wait(ExclusiveGuard::raw_mut(self)) }
    }

    #[inline]
    fn wait_until(&mut self, cv: &Condvar, timeout: Instant) -> WaitTimeoutResult {
        unsafe {
            cv.raw
                .exc_wait_until(ExclusiveGuard::raw_mut(self), timeout)
        }
    }

    #[inline]
    fn wait_for(&mut self, cv: &Condvar, duration: Duration) -> WaitTimeoutResult {
        unsafe { cv.raw.exc_wait_for(ExclusiveGuard::raw_mut(self), duration) }
    }
}

impl<L: RawLockInfo + RawShareLock + Parkable, T: ?Sized> Wait for ShareGuard<'_, L, T> {
    #[inline]
    fn wait(&mut self, cv: &Condvar) {
        unsafe { cv.raw.shr_wait(ShareGuard::raw_mut(self)) }
    }

    #[inline]
    fn wait_until(&mut self, cv: &Condvar, timeout: Instant) -> WaitTimeoutResult {
        unsafe { cv.raw.shr_wait_until(ShareGuard::raw_mut(self), timeout) }
    }

    #[inline]
    fn wait_for(&mut self, cv: &Condvar, duration: Duration) -> WaitTimeoutResult {
        unsafe { cv.raw.shr_wait_for(ShareGuard::raw_mut(self), duration) }
    }
}
