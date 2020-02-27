use crate::RawLockInfo;
use crate::unique_lock::RawUniqueLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Global;

pub type Mutex<T> = crate::mutex::Mutex<Global, T>;

impl Global {
    pub const fn mutex<T>(value: T) -> Mutex<T> {
        unsafe { Mutex::from_raw_parts(Self, value) }
    }
    
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut<T: ?Sized>(value: &mut T) -> &mut Mutex<T> {
        unsafe {
            std::mem::transmute(value)
        }
    }
    
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn mutex_from_mut_slice<T>(value: &mut [T]) -> &mut [Mutex<T>] {
        unsafe {
            std::mem::transmute(value)
        }
    }
    
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn transpose<T>(value: &mut Mutex<[T]>) -> &mut [Mutex<T>] {
        unsafe {
            std::mem::transmute(value)
        }
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn addr(&self) -> usize {
        (self as *const _ as usize) % GLOBAL.len()
    }

    #[inline]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn contends_with(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }

    #[inline]
    pub fn will_contend<T: ?Sized, U: ?Sized>(a: &Mutex<T>, b: &Mutex<U>) -> bool {
        unsafe { a.raw().addr() == b.raw().addr() }
    }
}

type Lock = crate::mutex::simple::RawLock;

unsafe impl RawLockInfo for Global {
    const INIT: Self = Self;

    type UniqueGuardTraits = <Lock as RawLockInfo>::UniqueGuardTraits;
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

unsafe impl RawUniqueLock for Global {
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
