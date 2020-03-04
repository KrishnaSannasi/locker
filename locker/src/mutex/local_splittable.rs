//! a local (single-threaded) splittable lock

use std::cell::Cell;

/// a local (single-threaded) splittable raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<LocalSplitLock>;
/// a local (single-threaded) splittable mutex
pub type Mutex<T> = crate::mutex::Mutex<LocalSplitLock, T>;

/// a local (single-threaded) splittable lock
pub struct LocalSplitLock {
    lock_count: Cell<usize>,
}

impl LocalSplitLock {
    /// create a new local (single-threaded) splittable lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            lock_count: Cell::new(0),
        }
    }

    /// create a new local splittable raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a new local splittable mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl crate::mutex::RawMutex for LocalSplitLock {}
unsafe impl crate::RawLockInfo for LocalSplitLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for LocalSplitLock {
    #[inline]
    fn exc_lock(&self) {
        assert!(
            self.exc_try_lock(),
            "Can't lock a locked local exclusive lock"
        );
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        if self.lock_count.get() == 0 {
            self.lock_count.set(1);
            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        debug_assert!(
            self.lock_count.get() > 0,
            "tried to unlock an unlocked exc lock"
        );

        self.lock_count.set(self.lock_count.get() - 1);
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for LocalSplitLock {
    #[inline]
    unsafe fn exc_split(&self) {
        let (lock_count, overflow) = self.lock_count.get().overflowing_add(1);
        assert!(!overflow, "tried to split a local exc lock too many times");
        self.lock_count.set(lock_count);
    }
}
