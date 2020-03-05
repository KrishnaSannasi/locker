//! A global lock set that uses the [default rwlock lock](crate::rwlock::default)

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::rwlock::default::DefaultLock;
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

/// A global lock set that uses the [default rwlock lock](crate::rwlock::default)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalLock;

/// a global raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<GlobalLock>;
/// a global mutex
pub type Mutex<T> = crate::mutex::Mutex<GlobalLock, T>;
/// a global raw rwlock
pub type RawRwLock = crate::rwlock::raw::RwLock<GlobalLock>;
/// a global rwlock
pub type RwLock<T> = crate::rwlock::RwLock<GlobalLock, T>;

impl GlobalLock {
    /// create a new global raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self) }
    }

    /// create a new global mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// create a new global raw rwlock
    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self) }
    }

    /// create a new global rwlock
    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }

    /// create a new global mutex
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut<T: ?Sized>(value: &mut T) -> &mut Mutex<T> {
        unsafe { std::mem::transmute(value) }
    }

    /// Create a new global mutex
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut_slice<T>(value: &mut [T]) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    /// Transpose a global mutex containing a slice into a slice of global mutexes
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_transpose<T>(value: &mut Mutex<[T]>) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    /// Create a new global rwlock
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn rwlock_from_mut<T: ?Sized>(value: &mut T) -> &mut Mutex<T> {
        unsafe { std::mem::transmute(value) }
    }

    /// Create a new global rwlock
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn rwlock_from_mut_slice<T>(value: &mut [T]) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    /// Transpose a global rwlock containing a slice into a slice of global rwlock
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn rwlock_transpose<T>(value: &mut Mutex<[T]>) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn addr(&self) -> usize {
        (self as *const _ as usize) % GLOBALLOCK.len()
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn get(&self) -> &'static DefaultLock {
        &GLOBALLOCK[self.addr()]
    }

    /// Checks if two global locks will contend
    #[inline]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn will_contend(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }

    /// Checks if two global mutexes will contend
    #[inline]
    pub fn will_mutex_contend<T: ?Sized, U: ?Sized>(a: &Mutex<T>, b: &Mutex<U>) -> bool {
        a.raw().inner().addr() == b.raw().inner().addr()
    }

    /// Checks if two global rwlock will contend
    #[inline]
    pub fn will_rwlock_contend<T: ?Sized, U: ?Sized>(a: &RwLock<T>, b: &RwLock<U>) -> bool {
        a.raw().inner().addr() == b.raw().inner().addr()
    }
}

// 61 because it is a large prime number,
// this will reduce contention between unrelated locks
// because unrealated locks will be unlikely to pick up the same lock,
// even they are contigious in memory
#[rustfmt::skip]
static GLOBALLOCK: [DefaultLock; 61] = [
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),

    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(), DefaultLock::new(), DefaultLock::new(), DefaultLock::new(),
    DefaultLock::new(),
];

impl crate::mutex::RawMutex for GlobalLock {}
unsafe impl crate::rwlock::RawRwLock for GlobalLock {}
unsafe impl RawLockInfo for GlobalLock {
    const INIT: Self = Self;

    type ExclusiveGuardTraits = <DefaultLock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <DefaultLock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for GlobalLock {
    #[inline]
    fn exc_lock(&self) {
        self.get().exc_lock()
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        self.get().exc_try_lock()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        self.get().exc_unlock()
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        self.get().exc_bump()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl RawExclusiveLockFair for GlobalLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        self.get().exc_unlock_fair()
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        self.get().exc_bump_fair()
    }
}

unsafe impl RawShareLock for GlobalLock {
    fn shr_lock(&self) {
        self.get().shr_lock()
    }

    fn shr_try_lock(&self) -> bool {
        self.get().shr_try_lock()
    }

    unsafe fn shr_split(&self) {
        self.get().shr_split()
    }

    unsafe fn shr_unlock(&self) {
        self.get().shr_unlock()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl RawShareLockFair for GlobalLock {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        self.get().shr_unlock_fair()
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        self.get().shr_bump_fair()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::RawTimedLock for GlobalLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::exclusive_lock::RawExclusiveLockTimed for GlobalLock {
    fn exc_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.get().exc_try_lock_until(instant)
    }

    fn exc_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.get().exc_try_lock_for(duration)
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::share_lock::RawShareLockTimed for GlobalLock {
    fn shr_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.get().shr_try_lock_until(instant)
    }

    fn shr_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.get().shr_try_lock_for(duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contention() {
        let mtx = [GlobalLock::mutex([0; 61]), GlobalLock::mutex([0; 61])];

        let [ref a, ref b] = mtx;
        assert!(GlobalLock::will_mutex_contend(a, b));

        let _lock = a.lock();
        assert!(b.try_lock().is_none());
        drop(_lock);

        let rwlock = [GlobalLock::rwlock([0; 61]), GlobalLock::rwlock([0; 61])];

        let [ref a, ref b] = rwlock;
        assert!(GlobalLock::will_rwlock_contend(a, b));

        let _lock = a.write();
        assert!(b.try_write().is_none());
        drop(_lock);

        let _lock = a.read();
        assert!(b.try_write().is_none());
        drop(_lock);

        let _lock = a.read();
        assert!(b.try_read().is_some());
        drop(_lock);

        let _lock = a.write();
        assert!(b.try_read().is_none());
        drop(_lock);

        let mtx = [GlobalLock::mutex([0; 60]), GlobalLock::mutex([0; 60])];

        let [ref a, ref b] = mtx;
        assert!(!GlobalLock::will_mutex_contend(a, b));

        let _lock = a.lock();
        let _lock = b.lock();

        let rwlock = [GlobalLock::rwlock([0; 60]), GlobalLock::rwlock([0; 60])];

        let [ref a, ref b] = rwlock;
        assert!(!GlobalLock::will_rwlock_contend(a, b));

        let _lock = a.write();
        assert!(b.try_write().is_some());
        drop(_lock);

        let _lock = a.read();
        assert!(b.try_write().is_some());
        drop(_lock);

        let _lock = a.read();
        assert!(b.try_read().is_some());
        drop(_lock);

        let _lock = a.write();
        assert!(b.try_read().is_some());
        drop(_lock);
    }
}
