use std::cell::Cell;

pub type RawMutex = crate::mutex::raw::Mutex<RawLock>;
pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RawRwLock = crate::rwlock::raw::RwLock<RawLock>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

pub struct RawLock {
    state: Cell<usize>,
}

impl RawLock {
    const LOCK_BIT: usize = 0b01;
    const EXC_BIT: usize = 0b10;
    const COUNT: usize = !0b11;
    const INC: usize = !Self::COUNT + 1;

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

impl crate::mutex::RawMutex for RawLock {}
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
        let state = self.state.get();

        if state == 0 {
            // if unlocked

            self.state.set(Self::LOCK_BIT | Self::EXC_BIT | Self::INC);

            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        let state = self.state.get();

        if state & Self::COUNT == Self::INC {
            self.state.set(0);
        } else {
            self.state.set(state - Self::INC);
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for RawLock {
    unsafe fn exc_split(&self) {
        let state = self.state.get();

        let state = state
            .checked_add(Self::INC)
            .expect("tried to create too many shared locks");

        self.state.set(state);
    }
}

unsafe impl crate::share_lock::RawShareLock for RawLock {
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

        if state == 0 {
            // if unlocked
            self.state.set(Self::LOCK_BIT | Self::INC);
        } else if state & Self::EXC_BIT == 0 {
            // if share locked

            let state = state
                .checked_add(Self::INC)
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
            .checked_add(Self::INC)
            .expect("tried to create too many shared locks");

        self.state.set(state);
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        let state = self.state.get();

        if state & Self::COUNT == Self::INC {
            self.state.set(0);
        } else {
            self.state.set(state - Self::INC);
        }
    }

    #[inline]
    unsafe fn shr_bump(&self) {}
}
