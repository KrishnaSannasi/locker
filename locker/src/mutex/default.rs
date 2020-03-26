//! A default raw mutex lock

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::RawLockInfo;

/// A default raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<DefaultLock>;
/// A default mutex
pub type Mutex<T> = crate::mutex::Mutex<DefaultLock, T>;

#[cfg(feature = "parking_lot_core")]
type Lock = crate::mutex::adaptive::AdaptiveLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::mutex::spin::SpinLock;

/// A default mutex lock implementation
///
/// This implementation will be a spin-lock by default, but if
/// the `parking_lot_core` feature is enabled then it will use
/// an adaptive strategy
#[repr(transparent)]
pub struct DefaultLock(Lock);

impl DefaultLock {
    /// Create a new default mutex lock
    #[inline]
    pub const fn new() -> Self {
        Self(Lock::new())
    }

    /// Create a new raw mutex
    #[inline]
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new mutex
    #[inline]
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl crate::Init for DefaultLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for DefaultLock {}
unsafe impl RawLockInfo for DefaultLock {
    type ExclusiveGuardTraits = <Lock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <Lock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for DefaultLock {
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
unsafe impl RawExclusiveLockFair for DefaultLock {
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
impl crate::RawTimedLock for DefaultLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for DefaultLock {
    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.0.exc_try_lock_until(instant)
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.0.exc_try_lock_for(duration)
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::condvar::Parkable for DefaultLock {}
