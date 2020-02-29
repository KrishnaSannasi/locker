use std::cell::Cell;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

pub struct RawLock {
    lock: Cell<u8>,
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

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        assert!(
            self.uniq_try_lock(),
            "Can't lock a locked local exclusive lock"
        );
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        !self.lock.replace(true)
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        debug_assert!(self.lock.get(), "tried to unlock an unlocked uniq lock");

        self.lock.set(false);
    }

    #[inline]
    unsafe fn uniq_bump(&self) {}
}
