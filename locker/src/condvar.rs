use parking_lot_core::{
    self, ParkResult, RequeueOp, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN,
};

use crate::exclusive_lock::{ExclusiveGuard, RawExclusiveLock};
use crate::share_lock::{RawShareLock, ShareGuard};

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

mod raw;
pub use raw::RawCondvar;

pub struct Condvar<L> {
    raw: RawCondvar<L>,
}

/// # Safety
///
/// `uniq_unlock` cannot call `parking_lot_core::park`, or panic
pub unsafe trait Parkable {
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
            raw: RawCondvar::new(),
        }
    }
}

impl<L: Parkable> Condvar<L> {
    #[inline]
    pub fn notify_one(&self) -> bool {
        self.raw.notify_one()
    }

    #[inline]
    pub fn notify_all(&self) -> usize {
        self.raw.notify_all()
    }

    #[inline]
    pub fn wait<W: Wait<Lock = L> + ?Sized>(&self, guard: &mut W) {
        guard.wait(self)
    }

    #[inline]
    pub fn wait_until<W: Wait<Lock = L> + ?Sized>(
        &self,
        guard: &mut W,
        instant: Instant,
    ) -> WaitTimeoutResult {
        guard.wait_until(self, instant)
    }

    #[inline]
    pub fn wait_for<W: Wait<Lock = L> + ?Sized>(
        &self,
        guard: &mut W,
        duration: Duration,
    ) -> WaitTimeoutResult {
        guard.wait_for(self, duration)
    }
}

pub trait Wait {
    type Lock;

    fn wait(&mut self, cv: &Condvar<Self::Lock>);

    fn wait_until(&mut self, cv: &Condvar<Self::Lock>, timeout: Instant) -> WaitTimeoutResult;

    fn wait_for(&mut self, cv: &Condvar<Self::Lock>, duration: Duration) -> WaitTimeoutResult;
}

impl<L: RawLockInfo + RawExclusiveLock + Parkable, T: ?Sized> Wait for ExclusiveGuard<'_, L, T> {
    type Lock = L;

    #[inline]
    fn wait(&mut self, cv: &Condvar<Self::Lock>) {
        unsafe { cv.raw.uniq_wait(self.raw_mut()) }
    }

    #[inline]
    fn wait_until(&mut self, cv: &Condvar<Self::Lock>, timeout: Instant) -> WaitTimeoutResult {
        unsafe { cv.raw.uniq_wait_until(self.raw_mut(), timeout) }
    }

    #[inline]
    fn wait_for(&mut self, cv: &Condvar<Self::Lock>, duration: Duration) -> WaitTimeoutResult {
        unsafe { cv.raw.uniq_wait_for(self.raw_mut(), duration) }
    }
}

impl<L: RawLockInfo + RawShareLock + Parkable, T: ?Sized> Wait for ShareGuard<'_, L, T> {
    type Lock = L;

    #[inline]
    fn wait(&mut self, cv: &Condvar<Self::Lock>) {
        unsafe { cv.raw.shr_wait(self.raw_mut()) }
    }

    #[inline]
    fn wait_until(&mut self, cv: &Condvar<Self::Lock>, timeout: Instant) -> WaitTimeoutResult {
        unsafe { cv.raw.shr_wait_until(self.raw_mut(), timeout) }
    }

    #[inline]
    fn wait_for(&mut self, cv: &Condvar<Self::Lock>, duration: Duration) -> WaitTimeoutResult {
        unsafe { cv.raw.shr_wait_for(self.raw_mut(), duration) }
    }
}
