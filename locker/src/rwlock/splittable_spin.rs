//! a splittable spin lock

use crate::spin_wait::SpinWait;

use std::sync::atomic::{AtomicUsize, Ordering};

/// a splittable spin raw mutex
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable rwlock lock](crate::rwlock::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawMutex = crate::mutex::raw::Mutex<SplitSpinLock>;

/// a splittable spin mutex
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable rwlock lock](crate::rwlock::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type Mutex<T> = crate::mutex::Mutex<SplitSpinLock, T>;

/// a splittable spin raw rwlock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable rwlock lock](crate::rwlock::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RawRwLock = crate::rwlock::raw::RwLock<SplitSpinLock>;

/// a splittable spin rwlock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable rwlock lock](crate::rwlock::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub type RwLock<T> = crate::rwlock::RwLock<SplitSpinLock, T>;

const EXC_BIT: usize = 1;
const INC: usize = 0b10;

/// a splittable spin lock
///
/// This lock can maintain multiple exclusive locks at the same time, thus allowing
/// you to call `ExclusiveGuard::split_map` and `ExclusiveGuard::try_split_map`
///
/// It is not reccomended to use this type in libraries,
/// instead use [the default splittable rwlock lock](crate::rwlock::splittable_default)
/// because if any other crate in the dependency tree turns on
/// `parking_lot_core`, then you will automatically get adaptive strategys,
/// which are more efficient in the general case. All this without sacrificing
/// platforms that can't support adaptive strategys.
pub struct SplitSpinLock {
    state: AtomicUsize,
}

impl SplitSpinLock {
    #[inline]
    /// create a new splittable spin lock
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    /// create a new spin lock based raw splittable mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self::new()) }
    }

    /// create a new spin lock based splittable mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// create a new spin lock based raw splittable rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self::new()) }
    }

    /// create a new spin lock based splittable rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }

    #[cold]
    #[inline(never)]
    fn exc_lock_slow(&self) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Acquire);

        loop {
            // Grab the lock if it isn't locked, even if there is a queue on it
            if state == 0 {
                if let Err(x) = self.state.compare_exchange_weak(
                    0,
                    EXC_BIT | INC,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    state = x;
                } else {
                    return true;
                }
            } else {
                state = self.state.load(Ordering::Acquire);
            }

            spinwait.spin();
        }
    }

    #[cold]
    #[inline(never)]
    fn shr_lock_slow(&self) -> bool {
        let mut spinwait = SpinWait::new();
        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            if state & EXC_BIT == 0 {
                if let Some(next_state) = state.checked_add(INC) {
                    if let Err(x) = self.state.compare_exchange_weak(
                        state,
                        next_state,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        state = x;
                    } else {
                        return true;
                    }
                }
            } else {
                state = self.state.load(Ordering::Relaxed);
            }

            spinwait.spin();
        }
    }

    fn split(&self) {
        let mut state = self.state.load(Ordering::Relaxed);

        loop {
            let new_state = state
                .checked_add(INC)
                .expect("tried to split too many times");

            if let Err(x) = self.state.compare_exchange_weak(
                state,
                new_state,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                return;
            }
        }
    }

    #[inline]
    fn unlock(&self) {
        let mut state = self.state.load(Ordering::Acquire);

        // while not final lock
        while state >= INC * 2 {
            if let Err(x) = self.state.compare_exchange_weak(
                state,
                state - INC,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                state = x;
            } else {
                // if we were able to decrement, then we released our lock
                return;
            }
        }

        // last lock
        self.state.store(0, Ordering::Release);
    }
}

impl crate::mutex::RawMutex for SplitSpinLock {}
unsafe impl crate::rwlock::RawRwLock for SplitSpinLock {}
unsafe impl crate::RawLockInfo for SplitSpinLock {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();

    type ExclusiveGuardTraits = (crate::NoSend, crate::NoSync);
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl crate::exclusive_lock::RawExclusiveLock for SplitSpinLock {
    #[inline]
    fn exc_lock(&self) {
        if !self.exc_try_lock() {
            self.exc_lock_slow();
        }
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.state
            .compare_exchange(0, EXC_BIT | INC, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.unlock();
    }

    #[inline]
    unsafe fn exc_bump(&self) {}
}

unsafe impl crate::exclusive_lock::SplittableExclusiveLock for SplitSpinLock {
    unsafe fn exc_split(&self) {
        self.split()
    }
}

unsafe impl crate::share_lock::RawShareLock for SplitSpinLock {
    #[inline]
    fn shr_lock(&self) {
        if !self.shr_try_lock() {
            self.shr_lock_slow();
        }
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);

        if state & EXC_BIT != 0 {
            // if there is a exc lock, we can't acquire a lock
            false
        } else if let Some(new_state) = state.checked_add(INC) {
            self.state
                .compare_exchange(state, new_state, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
        } else {
            false
        }
    }

    #[inline]
    unsafe fn shr_split(&self) {
        self.split()
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.unlock();
    }

    #[inline]
    unsafe fn shr_bump(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_utils::sync::WaitGroup;

    #[test]
    fn test_writes() {
        static MTX: Mutex<u32> = SplitSpinLock::mutex(10);
        let wait = WaitGroup::new();

        let _lock = MTX.lock();
        assert!(MTX.try_lock().is_none());
        drop(_lock);

        let t = std::thread::spawn({
            let wait = wait.clone();
            move || {
                assert!(MTX.try_lock().is_none());

                wait.wait();

                let _lock = MTX.lock();

                assert_eq!(*_lock, 100);
            }
        });

        let mut _lock = MTX.lock();
        wait.wait();

        *_lock = 100;

        drop(_lock);

        t.join().unwrap();
    }

    #[test]
    fn test_reads() {
        use crate::exclusive_lock::ExclusiveGuard;

        let m = SplitSpinLock::rwlock((10, 0));

        {
            let _a = m.read();
            let _b = m.read();
            let _c = crate::share_lock::ShareGuard::clone(&_b);
        }

        {
            let _a = m.try_write().unwrap();
            let (_b, _c) = ExclusiveGuard::split_map(_a, |(x, y)| (x, y));
        }

        {
            let _a = m.try_write().unwrap();
            let (_b, _c) = ExclusiveGuard::split_map(_a, |(x, y)| (x, y));
        }

        {
            let _a = m.read();
            let _b = m.read();
            let _c = crate::share_lock::ShareGuard::clone(&_b);
        }
    }
}
