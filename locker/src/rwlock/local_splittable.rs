use std::cell::Cell;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

pub struct RawLock {
    state: Cell<usize>,
}

impl RawLock {
    const LOCK_BIT: usize = 0b01;
    const UNIQ_BIT: usize = 0b10;
    const COUNT: usize = !0b11;
    const INC: usize = !Self::COUNT + 1;

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: Cell::new(0),
        }
    }
    
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }
    
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        unsafe { RwLock::from_raw_parts(Self::new(), value) }
    }
}

unsafe impl crate::RawLockInfo for RawLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type UniqueGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::unique_lock::RawUniqueLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        assert!(self.uniq_try_lock(), "Can't lock a locked local lock");
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        let state  = self.state.get();

        if state == 0 {
            // if unlocked

            self.state.set(Self::LOCK_BIT | Self::UNIQ_BIT | Self::INC);

            true
        } else {
            false
        }
    }

    #[inline]
    unsafe fn uniq_unlock(&self) {
        let state = self.state.get();

        if state & Self::COUNT == Self::INC {
            self.state.set(0);
        } else {
            self.state.set(state - Self::INC);
        }
    }
}

unsafe impl crate::unique_lock::SplittableUniqueLock for RawLock {
    unsafe fn uniq_split(&self) {
        let state = self.state.get();

        let state = state.checked_add(Self::INC)
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
        let state  = self.state.get();

        if state == 0 {
            // if unlocked
            self.state.set(Self::LOCK_BIT | Self::INC);
        } else if state & Self::UNIQ_BIT == 0 {
            // if share locked

            let state = state.checked_add(Self::INC)
                .expect("tried to create too many shared locks");
            
            self.state.set(state);
        } else {
            return false
        }

        true
    }

    #[inline]
    unsafe fn shr_split(&self) {
        let state = self.state.get();

        let state = state.checked_add(Self::INC)
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
}
