//! a spin lock

use crate::spin_wait::SpinWait;
use core::sync::atomic::{AtomicUsize, Ordering};

const EXC_LOCK: usize = !0;

/// a raw mutex backed by a spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default rwlock lock](crate::rwlock::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawMutex = crate::mutex::raw::Mutex<SpinLock>;

/// a mutex backed by a spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default rwlock lock](crate::rwlock::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type Mutex<T> = crate::mutex::Mutex<SpinLock, T>;

/// a raw rwlock backed by a spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default rwlock lock](crate::rwlock::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawRwLock = crate::rwlock::raw::RwLock<SpinLock>;

/// a rwlock backed by a spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default rwlock lock](crate::rwlock::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RwLock<T> = crate::rwlock::RwLock<SpinLock, T>;

/// A spin lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the defaultrwlock lock](crate::rwlock::default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub struct SpinLock {
    state: AtomicUsize,
}

impl SpinLock {
    /// create a new spin lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
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

    /// create a new spin lock based raw rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    /// create a new spin lock based rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }

    #[cold]
    fn exc_lock_slow(&self) {
        let mut spin = SpinWait::new();

        loop {
            if self
                .state
                .compare_exchange_weak(0, EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }

            spin.spin();
        }
    }

    #[cold]
    fn shr_lock_slow(&self) {
        let mut spin = SpinWait::new();
        let state = self.state.load(Ordering::Relaxed);

        loop {
            if let Some(new_state) = state.checked_add(1) {
                if self
                    .state
                    .compare_exchange_weak(state, new_state, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }

            spin.spin();
        }
    }

    #[cold]
    fn upgrade_slow(&self) {
        let mut spin = SpinWait::new();

        loop {
            if self
                .state
                .compare_exchange_weak(1, EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }

            spin.spin();
        }
    }
}

impl crate::Init for SpinLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for SpinLock {}
unsafe impl crate::rwlock::RawRwLock for SpinLock {}
unsafe impl crate::RawLockInfo for SpinLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for SpinLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.exc_lock_slow()
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.state
            .compare_exchange(0, EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.state.store(0, Ordering::Release);
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        // there are never any parked threads in a spin lock
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockDowngrade for SpinLock {
    #[inline]
    unsafe fn downgrade(&self) {
        self.state.store(1, Ordering::Relaxed);
    }
}

unsafe impl crate::share_lock::RawShareLock for SpinLock {
    #[inline]
    fn shr_lock(&self) {
        if !self.shr_try_lock() {
            self.shr_lock_slow();
        }
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);

        if let Some(new_state) = state.checked_add(1) {
            self.state
                .compare_exchange(state, new_state, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        } else {
            false
        }
    }

    #[inline]
    unsafe fn shr_split(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            if let Some(new_state) = state.checked_add(1) {
                if let Err(x) = self.state.compare_exchange(
                    state,
                    new_state,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    break;
                }
            } else {
                panic!("Tried to create too many shared locks!");
            }
        }
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        let state = self.state.fetch_sub(1, Ordering::Release);
        debug_assert_ne!(state, 0, "Can't unlock an unlocked local lock");
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        // there are never any parked threads in a spin lock
    }
}

unsafe impl crate::share_lock::RawShareLockUpgrade for SpinLock {
    unsafe fn upgrade(&self) {
        if !self.try_upgrade() {
            self.upgrade_slow();
        }
    }

    unsafe fn try_upgrade(&self) -> bool {
        self.state
            .compare_exchange(1, EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
}
