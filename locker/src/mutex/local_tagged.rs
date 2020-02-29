use std::cell::Cell;

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

pub struct RawLock {
    state: Cell<u8>,
}

impl RawLock {
    const LOCK_BIT: u8 = 1;

    pub const TAG_BITS: u8 = 7;
    const SHIFT: u8 = 8 - Self::TAG_BITS;
    const MASK: u8 = !(!0 << Self::SHIFT);

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: Cell::new(0),
        }
    }

    #[inline]
    pub const fn with_tag(tag: u8) -> Self {
        Self {
            state: Cell::new(tag << Self::SHIFT),
        }
    }

    pub fn tag(&self) -> u8 {
        self.state.get() >> Self::SHIFT
    }

    pub fn and_tag(&self, tag: u8) -> u8 {
        let tag = tag << Self::SHIFT | Self::MASK;
        let state = self.state.get();

        self.state.set(state & tag);

        state >> Self::SHIFT
    }

    pub fn or_tag(&self, tag: u8) -> u8 {
        let tag = tag << Self::SHIFT;

        let state = self.state.get();

        self.state.set(state | tag);

        state >> Self::SHIFT
    }

    pub fn replace_tag(&self, tag: u8) -> u8 {
        let state = self.state.get();

        self.state.set((state & Self::MASK) | tag);

        state >> Self::SHIFT
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for RawLock {
    #[inline]
    fn exc_lock(&self) {
        assert!(
            self.exc_try_lock(),
            "Can't state a locked local exclusive state"
        );
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        let state = self.state.get();

        self.state.set(state | Self::LOCK_BIT);

        state & Self::LOCK_BIT == 0
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        debug_assert!(self.state.get() & Self::LOCK_BIT != 0, "tried to unlock an unlocked exc state");

        let state = self.state.get();

        self.state.set(state & !Self::LOCK_BIT);
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}
