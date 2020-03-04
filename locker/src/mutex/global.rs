//! A global lock set that uses the [default mutex lock](crate::mutex::default)

use super::default::DefaultLock;
use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::RawLockInfo;

/// A global lock set that uses the [default mutex lock](crate::mutex::default)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalLock;

/// a global raw mutex
pub type RawMutex = crate::mutex::raw::Mutex<GlobalLock>;
/// a global mutex
pub type Mutex<T> = crate::mutex::Mutex<GlobalLock, T>;

impl GlobalLock {
    /// Create a new global raw mutex
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self) }
    }

    /// Create a new global mutex
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    /// Create a new global mutex
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut<T: ?Sized>(value: &mut T) -> &mut Mutex<T> {
        unsafe { std::mem::transmute(value) }
    }

    /// Create a new global mutex
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut_slice<T>(value: &mut [T]) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    /// Create transpose a global mutex containing a slice into a slice of global mutexes
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_transpose<T>(value: &mut Mutex<[T]>) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn addr(&self) -> usize {
        (self as *const _ as usize) % GLOBAL.len()
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
}

// 61 because it is a large prime number,
// this will reduce contention between unrelated locks
// because unrealated locks will be unlikely to pick up the same lock,
// even they are contigious in memory
#[rustfmt::skip]
static GLOBAL: [DefaultLock; 61] = [
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
unsafe impl RawLockInfo for GlobalLock {
    const INIT: Self = Self;

    type ExclusiveGuardTraits = <DefaultLock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <DefaultLock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for GlobalLock {
    #[inline]
    fn exc_lock(&self) {
        GLOBAL[self.addr()].exc_lock()
    }

    #[inline]
    fn exc_try_lock(&self) -> bool {
        GLOBAL[self.addr()].exc_try_lock()
    }

    #[inline]
    unsafe fn exc_unlock(&self) {
        GLOBAL[self.addr()].exc_unlock()
    }

    #[inline]
    unsafe fn exc_bump(&self) {
        GLOBAL[self.addr()].exc_bump()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl RawExclusiveLockFair for GlobalLock {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        GLOBAL[self.addr()].exc_unlock_fair()
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        GLOBAL[self.addr()].exc_bump_fair()
    }
}

#[test]
fn test_contention() {
    let mtx = [GlobalLock::mutex([0; 61]), GlobalLock::mutex([0; 61])];

    let [ref a, ref b] = mtx;
    assert!(GlobalLock::will_mutex_contend(a, b));

    let _lock = a.lock();
    assert!(b.try_lock().is_none());
    drop(_lock);

    let mtx = [GlobalLock::mutex([0; 60]), GlobalLock::mutex([0; 60])];

    let [ref a, ref b] = mtx;
    assert!(!GlobalLock::will_mutex_contend(a, b));

    let _lock = a.lock();
    let _lock = b.lock();
}
