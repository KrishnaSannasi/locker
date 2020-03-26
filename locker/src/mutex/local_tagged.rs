//! a local (single-threaded) tagged lock

use core::cell::Cell;

/// a local (single-threaded) tagged raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<LocalTaggedLock>;
/// a local (single-threaded) tagged mutex
pub type Mutex<T> = crate::mutex::Mutex<LocalTaggedLock, T>;

/// a local (single-threaded) tagged lock
pub struct LocalTaggedLock {
    state: Cell<u8>,
}

impl LocalTaggedLock {
    const LOCK_BIT: u8 = 1;

    /// The number of bits that this mutex can store
    ///
    /// This is guaranteed to be exactly 7
    pub const TAG_BITS: u8 = 7;
    const SHIFT: u8 = 8 - Self::TAG_BITS;
    const MASK: u8 = !(!0 << Self::SHIFT);

    /// create a local (single-threaded) tagged lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: Cell::new(0),
        }
    }

    /// create a local (single-threaded) tagged lock with the given tag
    #[inline]
    pub const fn with_tag(tag: u8) -> Self {
        Self {
            state: Cell::new(tag << Self::SHIFT),
        }
    }

    /// Get the tag
    pub fn tag(&self) -> u8 {
        self.state.get() >> Self::SHIFT
    }

    /// perform a bit-wise and with the given tag and the stored tag
    ///
    /// returns the old tag
    pub fn and_tag(&self, tag: u8) -> u8 {
        let tag = tag << Self::SHIFT | Self::MASK;
        let state = self.state.get();

        self.state.set(state & tag);

        state >> Self::SHIFT
    }

    /// perform a bit-wise or with the given tag and the stored tag
    ///
    /// returns the old tag
    pub fn or_tag(&self, tag: u8) -> u8 {
        let tag = tag << Self::SHIFT;

        let state = self.state.get();

        self.state.set(state | tag);

        state >> Self::SHIFT
    }

    /// swap the tag with the given tag
    ///
    /// returns the old tag
    pub fn replace_tag(&self, tag: u8) -> u8 {
        let state = self.state.get();

        self.state.set((state & Self::MASK) | tag);

        state >> Self::SHIFT
    }
    /// set the tag with the given tag
    pub fn set_tag(&self, tag: u8) {
        self.replace_tag(tag);
    }

    /// Create a new raw tagged mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// Create a new tagged mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

impl crate::Init for LocalTaggedLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for LocalTaggedLock {}
unsafe impl crate::RawLockInfo for LocalTaggedLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = core::convert::Infallible;
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for LocalTaggedLock {
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
        debug_assert!(
            self.state.get() & Self::LOCK_BIT != 0,
            "tried to unlock an unlocked exc state"
        );

        let state = self.state.get();

        self.state.set(state & !Self::LOCK_BIT);
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}
