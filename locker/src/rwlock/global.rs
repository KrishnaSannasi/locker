use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Global;

pub type RawMutex = crate::mutex::raw::Mutex<Global>;
pub type Mutex<T> = crate::mutex::Mutex<Global, T>;
pub type RawRwLock = crate::rwlock::raw::RwLock<Global>;
pub type RwLock<T> = crate::rwlock::RwLock<Global, T>;

impl Global {
    pub const fn raw_mutex() -> RawMutex {
        unsafe { RawMutex::from_raw(Self) }
    }

    pub const fn mutex<T>(value: T) -> Mutex<T> {
        Mutex::from_raw_parts(Self::raw_mutex(), value)
    }

    pub const fn raw_rwlock() -> RawRwLock {
        unsafe { RawRwLock::from_raw(Self) }
    }

    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        RwLock::from_raw_parts(Self::raw_rwlock(), value)
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut<T: ?Sized>(value: &mut T) -> &mut Mutex<T> {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut_slice<T>(value: &mut [T]) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_transpose<T>(value: &mut Mutex<[T]>) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn rwlock_from_mut<T: ?Sized>(value: &mut T) -> &mut Mutex<T> {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn rwlock_from_mut_slice<T>(value: &mut [T]) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn rwlock_transpose<T>(value: &mut Mutex<[T]>) -> &mut [Mutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn addr(&self) -> usize {
        (self as *const _ as usize) % GLOBAL.len()
    }

    #[inline]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn will_contend(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }

    #[inline]
    pub fn will_mutex_contend<T: ?Sized, U: ?Sized>(a: &Mutex<T>, b: &Mutex<U>) -> bool {
        a.raw().inner().addr() == b.raw().inner().addr()
    }

    #[inline]
    pub fn will_rwlock_contend<T: ?Sized, U: ?Sized>(a: &RwLock<T>, b: &RwLock<U>) -> bool {
        a.raw().inner().addr() == b.raw().inner().addr()
    }
}

#[cfg(feature = "parking_lot_core")]
type Lock = crate::rwlock::simple::RawLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::rwlock::spin::RawLock;

// 61 because it is a large prime number,
// this will reduce contention between unrelated locks
// because unrealated locks will be unlikely to pick up the same lock,
// even they are contigious in memory
#[rustfmt::skip]
static GLOBAL: [Lock; 61] = [
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),

    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(), Lock::new(), Lock::new(), Lock::new(),
    Lock::new(),
];

impl crate::mutex::RawMutex for Global {}
unsafe impl crate::rwlock::RawRwLock for Global {}
unsafe impl RawLockInfo for Global {
    const INIT: Self = Self;

    type ExclusiveGuardTraits = <Lock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <Lock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawExclusiveLock for Global {
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
unsafe impl RawExclusiveLockFair for Global {
    #[inline]
    unsafe fn exc_unlock_fair(&self) {
        GLOBAL[self.addr()].exc_unlock_fair()
    }

    #[inline]
    unsafe fn exc_bump_fair(&self) {
        GLOBAL[self.addr()].exc_bump_fair()
    }
}

unsafe impl RawShareLock for Global {
    fn shr_lock(&self) {
        GLOBAL[self.addr()].shr_lock()
    }

    fn shr_try_lock(&self) -> bool {
        GLOBAL[self.addr()].shr_try_lock()
    }

    unsafe fn shr_split(&self) {
        GLOBAL[self.addr()].shr_split()
    }

    unsafe fn shr_unlock(&self) {
        GLOBAL[self.addr()].shr_unlock()
    }
}

#[cfg(feature = "parking_lot_core")]
unsafe impl RawShareLockFair for Global {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        GLOBAL[self.addr()].shr_unlock_fair()
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        GLOBAL[self.addr()].shr_bump_fair()
    }
}

#[test]
fn test_contention() {
    let mtx = [Global::mutex([0; 61]), Global::mutex([0; 61])];

    let [ref a, ref b] = mtx;
    assert!(Global::will_mutex_contend(a, b));

    let _lock = a.lock();
    assert!(b.try_lock().is_none());
    drop(_lock);

    let rwlock = [Global::rwlock([0; 61]), Global::rwlock([0; 61])];

    let [ref a, ref b] = rwlock;
    assert!(Global::will_rwlock_contend(a, b));

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

    let mtx = [Global::mutex([0; 60]), Global::mutex([0; 60])];

    let [ref a, ref b] = mtx;
    assert!(!Global::will_mutex_contend(a, b));

    let _lock = a.lock();
    let _lock = b.lock();

    let rwlock = [Global::rwlock([0; 60]), Global::rwlock([0; 60])];

    let [ref a, ref b] = rwlock;
    assert!(!Global::will_rwlock_contend(a, b));

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
