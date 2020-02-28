use crate::share_lock::RawShareLock;
use crate::exclusive_lock::RawExclusiveLock;
use crate::RawLockInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Global;

pub type Mutex<T> = crate::mutex::Mutex<Global, T>;
pub type RwLock<T> = crate::rwlock::RwLock<Global, T>;

impl Global {
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self, value) }
    }

    pub const fn rwlock<T>(value: T) -> RwLock<T> {
        unsafe { RwLock::from_raw_parts(Self, value) }
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
        unsafe { a.raw().addr() == b.raw().addr() }
    }

    #[inline]
    pub fn will_rwlock_contend<T: ?Sized, U: ?Sized>(a: &RwLock<T>, b: &RwLock<U>) -> bool {
        unsafe { a.raw().addr() == b.raw().addr() }
    }
}

#[cfg(feature = "parking_lot_core")]
type Lock = crate::rwlock::simple::RawLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::rwlock::spin::RawLock;

unsafe impl RawLockInfo for Global {
    const INIT: Self = Self;

    type ExclusiveGuardTraits = <Lock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <Lock as RawLockInfo>::ShareGuardTraits;
}

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

unsafe impl RawExclusiveLock for Global {
    fn uniq_lock(&self) {
        GLOBAL[self.addr()].uniq_lock()
    }

    fn uniq_try_lock(&self) -> bool {
        GLOBAL[self.addr()].uniq_try_lock()
    }

    unsafe fn uniq_unlock(&self) {
        GLOBAL[self.addr()].uniq_unlock()
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
