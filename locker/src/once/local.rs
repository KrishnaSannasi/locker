use std::cell::Cell;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type Once = crate::once::Once<RawLock>;
pub type OnceCell<T> = crate::once::OnceCell<RawLock, T>;
pub type Lazy<T, F> = crate::once::Lazy<RawLock, T, F, crate::once::Panic>;
pub type RertyLazy<T, F> = crate::once::Lazy<RawLock, T, F, crate::once::Retry>;

pub struct RawLock {
    state: Cell<u8>,
}

impl RawLock {
    const LOCK_BIT: u8 = 0b001;
    const DONE_BIT: u8 = 0b010;
    const POISON_BIT: u8 = 0b100;

    pub const fn new() -> Self {
        RawLock {
            state: Cell::new(0),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }

    pub const fn once() -> Once {
        unsafe { Once::from_raw(Self::new()) }
    }

    pub const fn once_cell<T>() -> OnceCell<T> {
        unsafe { OnceCell::from_once(Self::once()) }
    }

    pub const fn lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }

    pub const fn retry_lazy<T, F>(func: F) -> Lazy<T, F> {
        unsafe { Lazy::from_raw_parts(Self::once(), func) }
    }
}

unsafe impl crate::once::Finish for RawLock {
    #[inline]
    fn is_done(&self) -> bool {
        (self.state.get() & Self::LOCK_BIT) != 0
    }

    #[inline]
    fn mark_done(&self) {
        let state = self.state.get();

        self.state.set(state | Self::DONE_BIT);
    }

    #[inline]
    fn get_and_mark_poisoned(&self) -> bool {
        let state = self.state.get();

        self.state.set(state | Self::POISON_BIT);

        (state & Self::POISON_BIT) != 0
    }
}

unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        assert!(
            self.uniq_try_lock(),
            "Can't lock a locked local exclusive lock"
        );
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        let state = self.state.get();

        self.state.set(state | Self::LOCK_BIT);

        (state & Self::LOCK_BIT) == 0
    }

    /// # Safety
    ///
    /// This exclusive lock must be locked before calling this function
    #[inline]
    unsafe fn uniq_unlock(&self) {
        let state = self.state.get();

        debug_assert!(
            (state & Self::LOCK_BIT) != 0,
            "tried to unlock an unlocked uniq lock"
        );

        self.state.set(state & !Self::LOCK_BIT);
    }
}
