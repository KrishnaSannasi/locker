//! a local (single-threaded) splittable rwlock lock

use std::cell::Cell;

const EXC_BIT: usize = 0b01;
const INC: usize = 0b10;

/// a local (single-threaded) splittable raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<LocalSplitLock>;
/// a local (single-threaded) splittable mutex
pub type Mutex<T> = crate::mutex::Mutex<LocalSplitLock, T>;
/// a local (single-threaded) splittable raw rwlock
pub type RawRwLock = crate::rwlock::raw::RwLock<LocalSplitLock>;
/// a local (single-threaded) splittable rwlock
pub type RwLock<T> = crate::rwlock::RwLock<LocalSplitLock, T>;

/// a local (single-threaded) splittable rwlock lock
pub struct LocalSplitLock {
    state: Cell<usize>,
}

impl LocalSplitLock {
    /// create a new local (single-threaded) splittable lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: Cell::new(0),
        }
    }

    /// create a new local (single-threaded) splittable raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a new local (single-threaded) splittable mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// create a new local (single-threaded) splittable raw rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    /// create a new local (single-threaded) splittable rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }
}

impl crate::Init for LocalSplitLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for LocalSplitLock {}
unsafe impl crate::rwlock::RawRwLock for LocalSplitLock {}
unsafe impl crate::RawLockInfo for LocalSplitLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for LocalSplitLock {
    #[inline]
    fn exc_lock(&self) {
        assert!(self.exc_try_lock(), "Can't lock a locked local lock");
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        let state = self.state.get();

        if state == 0 {
            // if unlocked

            self.state.set(EXC_BIT | INC);

            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.state.set(self.state.get().saturating_sub(INC));
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for LocalSplitLock {
    unsafe fn exc_split(&self) {
        let state = self.state.get();

        let state = state
            .checked_add(INC)
            .expect("tried to split the exclusive lock too many times");

        self.state.set(state);
    }
}

unsafe impl crate::share_lock::RawShareLock for LocalSplitLock {
    #[inline]
    fn shr_lock(&self) {
        assert!(
            self.shr_try_lock(),
            "Can't lock a unqiuely locked local lock"
        );
    }

    // #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.get();

        if state & EXC_BIT == 0 {
            // if share locked

            let state = state
                .checked_add(INC)
                .expect("tried to create too many shared locks");

            self.state.set(state);
        } else {
            return false;
        }

        true
    }

    #[inline]
    unsafe fn shr_split(&self) {
        let state = self.state.get();

        let state = state
            .checked_add(INC)
            .expect("tried to create too many shared locks");

        self.state.set(state);
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.state.set(self.state.get() - INC);
    }

    #[inline]
    unsafe fn shr_bump(&self) {}
}
