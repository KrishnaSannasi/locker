use std::cell::Cell;

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

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }
}

unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();

    type UniqueGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::unique_lock::RawUniqueLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        assert!(
            self.uniq_try_lock(),
            "Can't lock a locked local unique lock"
        );
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        !self.lock.replace(true)
    }

    /// # Safety
    ///
    /// This unique lock must be locked before calling this function
    #[inline]
    unsafe fn uniq_unlock(&self) {
        debug_assert!(self.lock.get(), "tried to unlock an unlocked uniq lock");

        self.lock.set(false);
    }
}
