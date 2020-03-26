//! a local (single threaded) mutex lock

use core::cell::Cell;

/// a local raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<LocalLock>;
/// a local mutex
pub type Mutex<T> = crate::mutex::Mutex<LocalLock, T>;

/// a local (single threaded) mutex lock
pub struct LocalLock {
    lock: Cell<bool>,
}

impl LocalLock {
    /// create a local mutex lock
    #[inline]
    pub const fn new() -> Self {
        LocalLock {
            lock: Cell::new(false),
        }
    }

    /// create a local raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a local mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}
impl crate::Init for LocalLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for LocalLock {}
unsafe impl crate::RawLockInfo for LocalLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = core::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for LocalLock {
    #[inline]
    fn exc_lock(&self) {
        assert!(
            self.exc_try_lock(),
            "Can't lock a locked local exclusive lock"
        );
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        !self.lock.replace(true)
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        debug_assert!(self.lock.get(), "tried to unlock an unlocked exc lock");

        self.lock.set(false);
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}
