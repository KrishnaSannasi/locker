//! a local (single-threaded) rwlock lock

use std::cell::Cell;

const EXC_LOCK: usize = !0;

/// a local (single-threaded) raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<LocalLock>;
/// a local (single-threaded) mutex
pub type Mutex<T> = crate::mutex::Mutex<LocalLock, T>;
/// a local (single-threaded) raw rwlock
pub type RawRwLock = crate::rwlock::raw::RwLock<LocalLock>;
/// a local (single-threaded) rwlock
pub type RwLock<T> = crate::rwlock::RwLock<LocalLock, T>;

/// a local (single-threaded) rwlock lock
pub struct LocalLock {
    state: Cell<usize>,
}

impl LocalLock {
    /// create a local (single-threaded) rwlock lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: Cell::new(0),
        }
    }

    /// create a local (single-threaded) raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a local (single-threaded) mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// create a local (single-threaded) raw rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    /// create a local (single-threaded) rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }
}

impl crate::Init for LocalLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for LocalLock {}
unsafe impl crate::rwlock::RawRwLock for LocalLock {}
unsafe impl crate::RawLockInfo for LocalLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for LocalLock {
    #[inline]
    fn exc_lock(&self) {
        assert!(self.exc_try_lock(), "Can't lock a locked local lock");
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        if self.state.get() == 0 {
            self.state.set(EXC_LOCK);
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

unsafe impl crate::share_lock::RawShareLock for LocalLock {
    #[inline]
    fn shr_lock(&self) {
        assert!(
            self.shr_try_lock(),
            "Can't lock a unqiuely locked local lock"
        );
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        if let Some(new_state) = self.state.get().checked_add(1) {
            self.state.set(new_state);
            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn shr_split(&self) {
        assert!(
            self.shr_try_lock(),
            "Tried to create too many shared locks!"
        );
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

unsafe impl crate::exclusive_lock::RawExclusiveLockDowngrade for LocalLock {
    unsafe fn downgrade(&self) {
        debug_assert_eq!(
            self.state.get(),
            EXC_LOCK,
            "cannot downgrade a shared lock!"
        );

        self.state.set(1);
    }
}

unsafe impl crate::share_lock::RawShareLockUpgrade for LocalLock {
    unsafe fn upgrade(&self) {
        assert!(
            self.try_upgrade(),
            "Cannot upgrade local shared lock while other local shared locks are active"
        );
    }

    unsafe fn try_upgrade(&self) -> bool {
        let state = self.state.get();

        if state == 1 {
            self.state.set(EXC_LOCK);

            true
        } else {
            false
        }
    }
}
