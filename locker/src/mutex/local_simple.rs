use std::cell::Cell;

pub type RawMutex = crate::mutex::raw::Mutex<RawLock>;
pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

pub struct RawLock {
    lock: Cell<bool>,
}

impl RawLock {
    #[inline]
    pub const fn new() -> Self {
        RawLock {
            lock: Cell::new(false),
        }
    }

    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
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
