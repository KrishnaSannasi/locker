use std::cell::Cell;

pub type RawMutex = crate::mutex::raw::Mutex<RawLock>;
pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RawRwLock = crate::rwlock::raw::RwLock<RawLock>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

pub struct RawLock {
    state: Cell<usize>,
}

impl RawLock {
    const EXC_LOCK: usize = usize::max_value();

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: Cell::new(0),
        }
    }

    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
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
        assert!(self.exc_try_lock(), "Can't lock a locked local lock");
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        if self.state.get() == 0 {
            self.state.set(Self::EXC_LOCK);
            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.state.set(0);
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

unsafe impl crate::share_lock::RawShareLock for RawLock {
    #[inline]
    fn shr_lock(&self) {
        assert!(
            self.shr_try_lock(),
            "Can't lock a unqiuely locked local lock"
        );
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.get();
        if state != Self::EXC_LOCK {
            self.state.set(state + 1);
            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn shr_split(&self) {
        let was_locked = self.shr_try_lock();
        assert!(was_locked, "Tried to create too many shared locks!");
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        let (state, ovf) = self.state.get().overflowing_sub(1);
        debug_assert!(!ovf, "Can't unlock an unlocked local lock");
        self.state.set(state);
    }

    #[inline]
    unsafe fn shr_bump(&self) {}
}

unsafe impl crate::exclusive_lock::RawExclusiveLockDowngrade for RawLock {
    unsafe fn downgrade(&self) {
        self.state.set(1);
    }
}
