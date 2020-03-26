//! A default tagged raw mutex

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::RawLockInfo;
use std::sync::atomic::Ordering;

/// A default tagged raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<TaggedDefaultLock>;
/// A default tagged mutex
pub type Mutex<T> = crate::mutex::Mutex<TaggedDefaultLock, T>;

#[cfg(feature = "parking_lot_core")]
type Lock = crate::mutex::tagged::TaggedLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::mutex::tagged_spin::TaggedSpinLock;

/// A tagged lock that can store up to `TAG_BITS` bits in the lower bits of the lock
///
/// This implementation will be a spin-lock by default, but if
/// the `parking_lot_core` feature is enabled then it will use
/// an adaptive strategy
#[repr(transparent)]
pub struct TaggedDefaultLock(Lock);

impl TaggedDefaultLock {
    /// The number of bits that this mutex can store
    ///
    /// This is guaranteed to be at least 4
    pub const TAG_BITS: u8 = Lock::TAG_BITS;

    /// Create a new default mutex lock
    pub const fn new() -> Self {
        Self(Lock::new())
    }

    /// create a new tagged spin lock with the given inital tag
    #[inline]
    pub const fn with_tag(tag: u8) -> Self {
        Self(Lock::with_tag(tag))
    }

    /// Get the tag with the specified load ordering
    pub fn tag(&self, order: Ordering) -> u8 {
        self.0.tag(order)
    }

    /// perform a bit-wise and with the given tag and the stored tag using
    /// the specifed ordering
    ///
    /// returns the old tag
    ///
    /// this lowers to a single `fetch_and`
    pub fn and_tag(&self, tag: u8, order: Ordering) -> u8 {
        self.0.and_tag(tag, order)
    }

    /// perform a bit-wise or with the given tag and the stored tag using
    /// the specifed ordering
    ///
    /// returns the old tag
    ///
    /// this lowers to a single `fetch_or`
    pub fn or_tag(&self, tag: u8, order: Ordering) -> u8 {
        self.0.or_tag(tag, order)
    }

    /// swap the tag with the given tag using the specied ordering
    ///
    /// returns the old tag
    pub fn swap_tag(&self, tag: u8, order: Ordering) -> u8 {
        self.0.swap_tag(tag, order)
    }

    /// swap the tag with the given tag using the specied orderings
    pub fn exchange_tag(&self, tag: u8, success: Ordering, failure: Ordering) -> u8 {
        self.0.exchange_tag(tag, success, failure)
    }

    /// update the tag with the given function until it returns `None` or succeeds using the specied orderings
    pub fn update_tag(
        &self,
        success: Ordering,
        failure: Ordering,
        f: impl FnMut(u8) -> Option<u8>,
    ) -> Result<u8, u8> {
        self.0.update_tag(success, failure, f)
    }

    /// Create a new raw tagged mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new tagged mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl crate::Init for TaggedDefaultLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for TaggedDefaultLock {}
unsafe impl RawLockInfo for TaggedDefaultLock {
    type ExclusiveGuardTraits = <Lock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <Lock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for TaggedDefaultLock {
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
unsafe impl RawExclusiveLockFair for TaggedDefaultLock {
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
impl crate::RawTimedLock for TaggedDefaultLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for TaggedDefaultLock {
    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.0.exc_try_lock_until(instant)
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.0.exc_try_lock_for(duration)
    }
}
