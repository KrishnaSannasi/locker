//! a tagged spin lock

use crate::exclusive_lock::RawExclusiveLock;
use crate::spin_wait::SpinWait;
use core::sync::atomic::{AtomicU8, Ordering};

/// A tagged spin raw mutex that can store up to `TAG_BITS` bits in the lower bits of the lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default tagged mutex lock](crate::mutex::tagged_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawMutex = crate::mutex::raw::Mutex<TaggedSpinLock>;

/// A tagged spin mutex that can store up to `TAG_BITS` bits in the lower bits of the lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default tagged mutex lock](crate::mutex::tagged_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type Mutex<T> = crate::mutex::Mutex<TaggedSpinLock, T>;

#[inline]
fn strongest_failure_ordering(order: Ordering) -> Ordering {
    use Ordering::*;

    match order {
        Release => Relaxed,
        Relaxed => Relaxed,
        SeqCst => SeqCst,
        Acquire => Acquire,
        AcqRel => Acquire,
        _ => unreachable!(),
    }
}

/// A tagged spin lock that can store up to `TAG_BITS` bits in the lower bits of the lock
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default tagged mutex lock](crate::mutex::tagged_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub struct TaggedSpinLock {
    state: AtomicU8,
}

impl TaggedSpinLock {
    const LOCK_BIT: u8 = 0b1000_0000;

    /// The number of bits that this mutex can store
    ///
    /// This is guaranteed to be at least 4
    pub const TAG_BITS: u8 = (!Self::MASK).trailing_zeros() as u8;
    const MASK: u8 = !Self::LOCK_BIT;

    /// create a new tagged spin lock
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(0),
        }
    }

    /// create a new tagged spin lock with the given inital tag
    #[inline]
    pub const fn with_tag(tag: u8) -> Self {
        Self {
            state: AtomicU8::new(tag & Self::MASK),
        }
    }

    /// Get the tag with the specified load ordering
    pub fn tag(&self, order: Ordering) -> u8 {
        self.state.load(order) & Self::MASK
    }

    /// perform a bit-wise and with the given tag and the stored tag using
    /// the specifed ordering
    ///
    /// returns the old tag
    ///
    /// this lowers to a single `fetch_and`
    pub fn and_tag(&self, tag: u8, order: Ordering) -> u8 {
        let tag = (tag & Self::MASK) | !Self::MASK;

        self.state.fetch_and(tag, order) & Self::MASK
    }

    /// perform a bit-wise or with the given tag and the stored tag using
    /// the specifed ordering
    ///
    /// returns the old tag
    ///
    /// this lowers to a single `fetch_or`
    pub fn or_tag(&self, tag: u8, order: Ordering) -> u8 {
        let tag = tag & Self::MASK;

        self.state.fetch_or(tag, order) & Self::MASK
    }

    /// swap the tag with the given tag using the specied ordering
    ///
    /// returns the old tag
    pub fn swap_tag(&self, tag: u8, order: Ordering) -> u8 {
        self.exchange_tag(tag, order, strongest_failure_ordering(order))
    }

    /// swap the tag with the given tag using the specied orderings
    #[inline]
    pub fn exchange_tag(&self, tag: u8, success: Ordering, failure: Ordering) -> u8 {
        match self.update_tag(success, failure, move |_| Some(tag)) {
            Ok(x) => x,
            Err(_) => unreachable!(),
        }
    }

    /// update the tag with the given function until it returns `None` or succeeds using the specied orderings
    pub fn update_tag(
        &self,
        success: Ordering,
        failure: Ordering,
        mut f: impl FnMut(u8) -> Option<u8>,
    ) -> Result<u8, u8> {
        let mut state = self.state.load(failure);

        while let Some(tag) = f(state & Self::MASK) {
            match self.state.compare_exchange_weak(
                state,
                (state & !Self::MASK) | (tag & Self::MASK),
                success,
                failure,
            ) {
                Err(x) => state = x,
                Ok(x) => return Ok(x & Self::MASK),
            }
        }

        Err(state & Self::MASK)
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

impl crate::Init for TaggedSpinLock {
    const INIT: Self = Self::new();
}

unsafe impl crate::mutex::RawMutex for TaggedSpinLock {}
unsafe impl crate::RawLockInfo for TaggedSpinLock {
    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = core::convert::Infallible;
}

unsafe impl RawExclusiveLock for TaggedSpinLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.lock_slow();
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Relaxed);

        (state & Self::LOCK_BIT == 0)
            && self
                .state
                .compare_exchange(
                    state,
                    state | Self::LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        debug_assert_ne!(state & Self::LOCK_BIT, 0);

        while let Err(x) = self.state.compare_exchange_weak(
            state,
            state & !Self::LOCK_BIT,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            state = x;
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

impl TaggedSpinLock {
    #[cold]
    fn lock_slow(&self) {
        let mut state = self.state.load(Ordering::Relaxed);
        let mut spin = SpinWait::new();

        loop {
            spin.spin();

            if state & Self::LOCK_BIT == 0 {
                continue;
            }

            if let Err(x) = self.state.compare_exchange(
                state,
                state | Self::LOCK_BIT,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                break;
            }
        }
    }
}
