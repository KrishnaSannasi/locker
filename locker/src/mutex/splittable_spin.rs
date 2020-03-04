//! a splittable spin lock

use crate::exclusive_lock::RawExclusiveLock;
use crate::spin_wait::SpinWait;
use std::sync::atomic::{AtomicUsize, Ordering};

/// a splittable spin raw mutex
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable mutex lock](crate::mutex::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawMutex = crate::mutex::raw::Mutex<SplitSpinLock>;

/// a splittable spin mutex
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable mutex lock](crate::mutex::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type Mutex<T> = crate::mutex::Mutex<SplitSpinLock, T>;

const INC: usize = 1;

/// a splittable spin lock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable mutex lock](crate::mutex::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub struct SplitSpinLock {
    state: AtomicUsize,
}

impl SplitSpinLock {
    /// create a new splittable spin lock
    pub const fn new() -> Self {
        SplitSpinLock {
            state: AtomicUsize::new(0),
        }
    }

    /// create a new splittable raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a new splittable mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl SplitSpinLock {
    #[cold]
    #[inline(never)]
    fn lock_slow(&self) {
        let mut wait = SpinWait::new();

        while self
            .state
            .compare_exchange_weak(0, INC, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            wait.spin();
        }
    }
}

impl crate::mutex::RawMutex for SplitSpinLock {}
unsafe impl crate::RawLockInfo for SplitSpinLock {
    const INIT: Self = Self::new();
    type ExclusiveGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for SplitSpinLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.lock_slow();
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        0 == self.state.compare_and_swap(0, INC, Ordering::Acquire)
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        while let Err(x) = self.state.compare_exchange_weak(
            state,
            state - INC,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            state = x;
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for SplitSpinLock {
    unsafe fn exc_split(&self) {
        self.state.fetch_add(INC, Ordering::Relaxed);
    }
}
