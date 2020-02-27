use std::cell::Cell;

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
        if self.lock_count.get() == 0 {
            self.lock_count.set(1);
            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        debug_assert!(
            self.lock_count.get() > 0,
            "tried to unlock an unlocked uniq lock"
        );

        self.lock_count.set(self.lock_count.get() - 1);
    }
}

unsafe impl crate::unique_lock::SplittableUniqueLock for RawLock {
    #[inline]
    unsafe fn uniq_split(&self) {
        let (lock_count, overflow) = self.lock_count.get().overflowing_add(1);
        assert!(!overflow, "tried to split a local uniq lock too many times");
        self.lock_count.set(lock_count);
    }
}
