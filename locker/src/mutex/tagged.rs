//! a tagged lock

use crate::exclusive_lock::RawExclusiveLock;
use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

// UnparkToken used to indicate that that the target thread should attempt to
// state the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

/// A tagged raw mutex that can store up to `TAG_BITS` bits in the lower bits of the lock
pub type RawMutex = crate::mutex::raw::Mutex<TaggedLock>;

/// A tagged mutex that can store up to `TAG_BITS` bits in the lower bits of the lock
pub type Mutex<T> = crate::mutex::Mutex<TaggedLock, T>;

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

/// A tagged lock that can store up to `TAG_BITS` bits in the lower bits of the lock
pub struct TaggedLock {
    state: AtomicU8,
}

impl TaggedLock {
    const LOCK_BIT: u8 = 0b1000_0000;
    const PARK_BIT: u8 = 0b0100_0000;

    /// The number of bits that this mutex can store
    ///
    /// This is guaranteed to be at least 4
    pub const TAG_BITS: u8 = (!Self::MASK).trailing_zeros() as u8;
    const MASK: u8 = !(Self::LOCK_BIT | Self::PARK_BIT);

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

impl crate::mutex::RawMutex for TaggedLock {}
unsafe impl crate::RawLockInfo for TaggedLock {
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl RawExclusiveLock for TaggedLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.lock_slow(None);
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

        if state & Self::PARK_BIT == 0 {
            while let Err(x) = self.state.compare_exchange_weak(
                state,
                state & !Self::LOCK_BIT,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            }
        } else {
            self.unlock_slow(false);
        }
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        let state = self.state.load(Ordering::Relaxed);

        debug_assert_ne!(state & Self::LOCK_BIT, 0);

        if state & Self::PARK_BIT != 0 {
            self.bump_slow(false);
        }
    }
}

unsafe impl crate::exclusive_lock::RawExclusiveLockFair for TaggedLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        debug_assert_ne!(state & Self::LOCK_BIT, 0);

        if state & Self::PARK_BIT == 0 {
            while let Err(x) = self.state.compare_exchange_weak(
                state,
                state & !Self::LOCK_BIT,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            }
        } else {
            self.unlock_slow(true);
        }
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        let state = self.state.load(Ordering::Relaxed);

        debug_assert_ne!(state & Self::LOCK_BIT, 0);

        if state & Self::PARK_BIT != 0 {
            self.bump_slow(true);
        }
    }
}
impl TaggedLock {
    #[cold]
    #[inline(never)]
    fn lock_slow(&self, timeout: Option<Instant>) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);
        loop {
            // Grab the state if it isn't locked, even if there is a queue on it
            if state & Self::LOCK_BIT == 0 {
                match self.state.compare_exchange_weak(
                    state,
                    state | Self::LOCK_BIT,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return true,
                    Err(x) => state = x,
                }
                continue;
            }

            // If there is no queue, try spinning a few times
            if state & Self::PARK_BIT == 0 && spinwait.spin() {
                state = self.state.load(Ordering::Relaxed);
                continue;
            }

            // Set the parked bit
            if state & Self::PARK_BIT == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    state,
                    state | Self::PARK_BIT,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    state = x;
                    continue;
                }
            }

            // Park our thread until we are woken up by an unlock
            let addr = self as *const _ as usize;
            let validate = || {
                self.state.load(Ordering::Relaxed) & !Self::MASK == Self::LOCK_BIT | Self::PARK_BIT
            };
            let before_sleep = || {};
            let timed_out = |_, was_last_thread| {
                // Clear the parked bit if we were the last parked thread
                if was_last_thread {
                    self.state.fetch_and(!Self::PARK_BIT, Ordering::Relaxed);
                }
            };

            // SAFETY:
            //   * `addr` is an address we control.
            //   * `validate`/`timed_out` does not panic or call into any function of `parking_lot`.
            //   * `before_sleep` does not call `park`, nor does it panic.
            match unsafe {
                parking_lot_core::park(
                    addr,
                    validate,
                    before_sleep,
                    timed_out,
                    DEFAULT_PARK_TOKEN,
                    timeout,
                )
            } {
                // The thread that unparked us passed the state on to us
                // directly without unlocking it.
                ParkResult::Unparked(TOKEN_HANDOFF) => return true,

                // We were unparked normally, try acquiring the state again
                ParkResult::Unparked(_) => (),

                // The validation function failed, try locking again
                ParkResult::Invalid => (),

                // Timeout expired
                ParkResult::TimedOut => return false,
            }

            // Loop back and try locking again
            spinwait.reset();
            state = self.state.load(Ordering::Relaxed);
        }
    }

    #[cold]
    #[inline(never)]
    fn unlock_slow(&self, force_fair: bool) {
        // Unpark one thread and leave the parked bit set if there might
        // still be parked threads on this address.
        let addr = self as *const _ as usize;
        let callback = |result: UnparkResult| {
            // If we are using a fair unlock then we should keep the
            // mutex locked and hand it off to the unparked thread.
            if result.unparked_threads != 0 && (force_fair || result.be_fair) {
                // Clear the parked bit if there are no more parked
                // threads.
                if !result.have_more_threads {
                    self.state.fetch_and(!Self::PARK_BIT, Ordering::Relaxed);
                }
                return TOKEN_HANDOFF;
            }

            // Clear the locked bit, and the parked bit as well if there
            // are no more parked threads.
            if result.have_more_threads {
                self.state.fetch_and(!Self::LOCK_BIT, Ordering::Release);
            } else {
                self.state.fetch_and(Self::MASK, Ordering::Release);
            }
            TOKEN_NORMAL
        };

        // SAFETY:
        //   * `addr` is an address we control.
        //   * `callback` does not panic or call into any function of `parking_lot`.
        unsafe {
            parking_lot_core::unpark_one(addr, callback);
        }
    }

    #[cold]
    fn bump_slow(&self, force_fair: bool) {
        self.unlock_slow(force_fair);
        self.exc_lock();
    }
}

impl crate::RawTimedLock for TaggedLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for TaggedLock {
    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        if self.exc_try_lock() {
            true
        } else {
            self.lock_slow(Some(instant))
        }
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        if self.exc_try_lock() {
            true
        } else {
            self.lock_slow(Instant::now().checked_add(duration))
        }
    }
}
