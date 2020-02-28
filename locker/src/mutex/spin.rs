use crate::spin_wait::SpinWait;
use std::sync::atomic::{AtomicBool, Ordering};

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub struct RawLock {
    lock: AtomicBool,
}

impl RawLock {
    #[inline]
    pub const fn new() -> Self {
        RawLock {
            lock: AtomicBool::new(false),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        let mut spin = SpinWait::new();

        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin.spin();
        }
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        self.lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    #[inline]
    unsafe fn uniq_bump(&self) {
        // there are never any parked threads in a spin lock
    }
}
