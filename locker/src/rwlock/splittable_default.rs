//! A default raw rwlock lock

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

/// A default raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<DefaultLock>;
/// A default mutex
pub type Mutex<T> = crate::mutex::Mutex<DefaultLock, T>;
/// A default raw mutex
pub type RawRwLock = crate::rwlock::raw::RwLock<DefaultLock>;
/// A default mutex
pub type RwLock<T> = crate::rwlock::RwLock<DefaultLock, T>;

#[cfg(feature = "parking_lot_core")]
type Lock = crate::rwlock::splittable::SplitLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::rwlock::splittable_spin::SplitSpinLock;

/// A default mutex lock implementation
///
/// This implementation will be a spin-lock by default, but if
/// the `parking_lot_core` feature is enabled then it will use
/// an adaptive strategy
#[repr(transparent)]
pub struct DefaultLock(Lock);

impl DefaultLock {
    /// Create a new default mutex lock
    pub const fn new() -> Self {
        Self(Lock::new())
    }

    /// Create a new raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// Create a new raw rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    /// Create a new rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }
}

impl crate::mutex::RawMutex for DefaultLock {}
unsafe impl RawLockInfo for DefaultLock {
    const INIT: Self = Self::new();

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

unsafe impl RawShareLock for DefaultLock {
    #[inline]
    fn shr_lock(&self) {
        self.0.shr_lock();
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        self.0.shr_try_lock()
    }

    unsafe fn shr_split(&self) {
        self.0.shr_split()
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.0.shr_unlock()
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        self.0.shr_bump()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl RawShareLockFair for DefaultLock {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        self.0.shr_unlock_fair()
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        self.0.shr_bump_fair()
    }
}
