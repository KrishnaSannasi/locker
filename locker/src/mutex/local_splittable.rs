use std::cell::Cell;

pub type RawMutex = crate::mutex::raw::Mutex<RawLock>;
pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

pub struct RawLock {
    lock_count: Cell<usize>,
}

impl RawLock {
    #[inline]
    pub const fn new() -> Self {
        Self {
            lock_count: Cell::new(0),
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

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for RawLock {
    #[inline]
    unsafe fn exc_split(&self) {
        let (lock_count, overflow) = self.lock_count.get().overflowing_add(1);
        assert!(!overflow, "tried to split a local exc lock too many times");
        self.lock_count.set(lock_count);
    }
}
