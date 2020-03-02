use crate::exclusive_lock::RawExclusiveLock;
use crate::spin_wait::SpinWait;
use std::sync::atomic::{AtomicU8, Ordering};

pub type RawMutex = crate::mutex::raw::Mutex<RawLock>;
pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

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

pub struct RawLock {
    state: AtomicU8,
}

impl RawLock {
    const LOCK_BIT: u8 = 0b1;

    pub const TAG_BITS: u8 = 7;
    const SHIFT: u8 = 8 - Self::TAG_BITS;
    const MASK: u8 = !(!0 << Self::SHIFT);

    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(0),
        }
    }

    #[inline]
    pub const fn with_tag(tag: u8) -> Self {
        Self {
            state: AtomicU8::new(tag << Self::SHIFT),
        }
    }

    pub fn tag(&self, order: Ordering) -> u8 {
        self.state.load(order) >> Self::SHIFT
    }

    pub fn and_tag(&self, tag: u8, order: Ordering) -> u8 {
        let tag = tag << Self::SHIFT | Self::MASK;

        self.state.fetch_and(tag, order) >> Self::SHIFT
    }

    pub fn or_tag(&self, tag: u8, order: Ordering) -> u8 {
        let tag = tag << Self::SHIFT;

        self.state.fetch_or(tag, order) >> Self::SHIFT
    }

    pub fn swap_tag(&self, tag: u8, order: Ordering) -> u8 {
        self.exchange_tag(tag, order, strongest_failure_ordering(order))
    }

    pub fn exchange_tag(&self, tag: u8, success: Ordering, failure: Ordering) -> u8 {
        let tag = tag << Self::SHIFT;
        let mut state = self.state.load(failure);

        while let Err(x) =
            self.state
                .compare_exchange_weak(state, (state & Self::MASK) | tag, success, failure)
        {
            state = x;
        }

        state >> Self::SHIFT
    }

    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }
}

unsafe impl crate::mutex::RawMutex for RawLock {}
unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for RawLock {
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

impl RawLock {
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
