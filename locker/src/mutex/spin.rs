//! a spin lock

use crate::spin_wait::SpinWait;
use core::sync::atomic::{AtomicBool, Ordering};

/// a raw mutex backed by a spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default mutex lock](crate::mutex::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawMutex = crate::mutex::raw::Mutex<SpinLock>;

/// a mutex backed by a spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default mutex lock](crate::mutex::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type Mutex<T> = crate::mutex::Mutex<SpinLock, T>;

/// A spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default mutex lock](crate::mutex::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub struct SpinLock {
    lock: AtomicBool,
}

impl SpinLock {
    /// create a new spin lock
    #[inline]
    pub const fn new() -> Self {
        SpinLock {
            lock: AtomicBool::new(false),
        }
    }

    /// create a new spin lock based raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a new spin lock based mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl crate::Init for SpinLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for SpinLock {}
unsafe impl crate::RawLockInfo for SpinLock {
    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = core::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for SpinLock {
    #[inline]
    fn exc_lock(&self) {
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
    fn exc_try_lock(&self) -> bool {
        self.lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        // there are never any parked threads in a spin lock
    }
}
