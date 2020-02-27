use parking_lot_core::{self, ParkResult, SpinWait, UnparkResult, UnparkToken, DEFAULT_PARK_TOKEN};

// UnparkToken used to indicate that that the target thread should attempt to
// lock the mutex again as soon as it is unparked.
const TOKEN_NORMAL: UnparkToken = UnparkToken(0);

// UnparkToken used to indicate that the mutex is being handed off to the target
// thread directly without unlocking it.
const TOKEN_HANDOFF: UnparkToken = UnparkToken(1);

use std::sync::atomic::{AtomicUsize, Ordering};

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;
pub type RwLock<T> = crate::rwlock::RwLock<RawLock, T>;

pub struct RawLock {
    state: AtomicUsize,
}

impl RawLock {
    const UNIQ_LOCK: usize = usize::max_value();

    pub const fn new() -> Self {
        Self { state: AtomicUsize::new(0) }
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
    
    type UniqueGuardTraits = ();
    type ShareGuardTraits = ();
}

unsafe impl crate::unique_lock::RawUniqueLock for RawLock {
    fn uniq_lock(&self) {
        if !self.uniq_try_lock() {
            self.uniq_lock_slow();
        }
    }

    fn uniq_try_lock(&self) -> bool {
        self.state.compare_exchange_weak(0 ,Self::UNIQ_LOCK, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    unsafe fn uniq_unlock(&self) {
        self.state.store(0, Ordering::Release);
    }
}

unsafe impl crate::share_lock::RawShareLock for RawLock {
    fn shr_lock(&self) {
        if !self.shr_try_lock() {
            self.shr_lock_slow()
        }
    }

    fn shr_try_lock(&self) -> bool {
        let mut spin = SpinWait::new();
        let mut state = self.state.load(Ordering::Acquire);

        loop {
            if state == Self::UNIQ_LOCK {
                return false;
            }

            if let Err(x) = self.state.compare_exchange_weak(state, state + 1, Ordering::Acquire, Ordering::Relaxed) {
                state = x
            } else {
                return true
            }

            if !spin.spin() {
                // timeout
                return false;
            }
        }
    }

    unsafe fn shr_split(&self) {
        let state = self.state.fetch_add(1, Ordering::Relaxed);
        debug_assert!(state != Self::UNIQ_LOCK, "Can't lock a unqiuely locked lock");
    }

    unsafe fn shr_unlock(&self) {
        let state = self.state.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(state != 0, "Can't unlock an unlocked lock");
    }
}

impl RawLock {
    #[cold]
    #[inline(never)]
    fn uniq_lock_slow(&self) {

    }
    
    #[cold]
    #[inline(never)]
    fn shr_lock_slow(&self) {

    }
}
