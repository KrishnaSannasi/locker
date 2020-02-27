use std::sync::atomic::{AtomicBool, Ordering, spin_loop_hint};

pub type Mutex<T> = crate::mutex::Mutex<RawLock, T>;

#[cfg(feature="parking_lot_core")]
use parking_lot_core::SpinWait;

// Wastes some CPU time for the given number of iterations,
// using a hint to indicate to the CPU that we are spinning.
#[inline]
#[cfg(not(feature="parking_lot_core"))]
fn cpu_relax(iterations: u32) {
    for _ in 0..iterations {
        spin_loop_hint()
    }
}

#[cfg(not(feature="parking_lot_core"))]
struct SpinWait {
    counter: u32,
}

#[cfg(not(feature="parking_lot_core"))]
impl SpinWait {
    /// Creates a new `SpinWait`.
    #[inline]
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Spins until the sleep threshold has been reached.
    ///
    /// This function returns whether the sleep threshold has been reached, at
    /// which point further spinning has diminishing returns and the thread
    /// should be parked instead.
    ///
    /// The spin strategy will initially use a CPU-bound loop but will fall back
    /// to yielding the CPU to the OS after a few iterations.
    #[inline]
    pub fn spin(&mut self) -> bool {
        self.counter = self.counter.min(9) + 1;
        
        #[cfg(feature = "std")]
        {
            if self.counter > 3 {
                std::thread::yield_now();
                return;
            }
        }
        
        cpu_relax(1 << self.counter);
        true
    }
}

pub struct RawLock {
    lock: AtomicBool,
}

impl RawLock {
    #[inline]
    pub const fn new() -> Self {
        RawLock {
            lock: AtomicBool::new(false),
        }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self::new(), value) }
    }
}

unsafe impl crate::RawLockInfo for RawLock {
    const INIT: Self = Self::new();
    
    type UniqueGuardTraits = ();
    type ShareGuardTraits = std::convert::Infallible;
}

unsafe impl crate::unique_lock::RawUniqueLock for RawLock {
    #[inline]
    fn uniq_lock(&self) {
        let mut spin = SpinWait::new();

        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            spin.spin();
        }
    }

    #[inline]
    fn uniq_try_lock(&self) -> bool {
        self.lock.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// # Safety
    ///
    /// This unique lock must be locked before calling this function
    #[inline]
    unsafe fn uniq_unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }
}
