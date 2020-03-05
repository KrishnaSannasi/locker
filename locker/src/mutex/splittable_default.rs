//! A default raw mutex

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair, SplittableExclusiveLock};
use crate::RawLockInfo;

/// A default raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<SplitDefaultLock>;
/// A default mutex
pub type Mutex<T> = crate::mutex::Mutex<SplitDefaultLock, T>;

#[cfg(feature = "parking_lot_core")]
type Lock = crate::mutex::splittable::SplitLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::mutex::splittable_spin::SplitSpinLock;

/// A default splittable mutex lock implementation
///
/// This implementation will be a spin-lock by default, but if
/// the `parking_lot_core` feature is enabled then it will use
/// an adaptive strategy
#[repr(transparent)]
pub struct SplitDefaultLock(Lock);

impl SplitDefaultLock {
    /// Create a new default splittable mutex lock
    pub const fn new() -> Self {
        Self(Lock::new())
    }

    /// Create a new raw splittable mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new splittable mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl crate::mutex::RawMutex for SplitDefaultLock {}
unsafe impl RawLockInfo for SplitDefaultLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = <Lock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <Lock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for SplitDefaultLock {
    #[inline]
    fn exc_lock(&self) {
        self.0.exc_lock();
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.0.exc_try_lock()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.0.exc_unlock()
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        self.0.exc_bump()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl RawExclusiveLockFair for SplitDefaultLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        self.0.exc_unlock_fair()
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        self.0.exc_bump_fair()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::RawTimedLock for SplitDefaultLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for SplitDefaultLock {
    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.0.exc_try_lock_until(instant)
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.0.exc_try_lock_for(duration)
    }
}

unsafe impl SplittableExclusiveLock for SplitDefaultLock {
    #[inline]
    unsafe fn exc_split(&self) {
        self.0.exc_split()
    }
}
