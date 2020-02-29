use crate::spin_wait::SpinWait;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

pub struct RawLock {
    state: AtomicUsize,
}

impl RawLock {
    const EXC_LOCK: usize = usize::max_value();

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }

    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        unsafe { RwLock::from_raw_parts(Self::new(), value) }
    }

    #[cold]
    fn exc_lock_slow(&self) {
        let mut spin = SpinWait::new();

        loop {
            if self
                .state
                .compare_exchange(0, Self::EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
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

        loop {
            if self
                .state
                .compare_exchange(0, Self::EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }

            spin.spin();
        }
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::rwlock::RawRwLock for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.exc_lock_slow()
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.state
            .compare_exchange(0, Self::EXC_LOCK, Ordering::Acquire, Ordering::Relaxed)
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

unsafe impl crate::exclusive_lock::RawExclusiveLockDowngrade for RawLock {
    #[inline]
    unsafe fn downgrade(&self) {
        self.state.store(1, Ordering::Relaxed);
    }
}

unsafe impl crate::share_lock::RawShareLock for RawLock {
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
