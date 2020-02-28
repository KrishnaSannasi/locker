use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Global;

pub type ReentrantMutex<T> = crate::reentrant::ReentrantMutex<Global, T>;

impl Global {
    pub const fn remutex<T>(value: T) -> ReentrantMutex<T> {
        unsafe { ReentrantMutex::from_raw_parts(Self, value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn remutex_from_mut<T: ?Sized>(value: &mut T) -> &mut ReentrantMutex<T> {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn remutex_from_mut_slice<T>(value: &mut [T]) -> &mut [ReentrantMutex<T>] {
        unsafe { std::mem::transmute(value) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn remutex_transpose<T>(value: &mut ReentrantMutex<[T]>) -> &mut [ReentrantMutex<T>] {
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
    pub fn will_remutex_contend<T: ?Sized, U: ?Sized>(
        a: &ReentrantMutex<T>,
        b: &ReentrantMutex<U>,
    ) -> bool {
        unsafe { a.raw().addr() == b.raw().addr() }
    }
}

#[cfg(feature = "parking_lot_core")]
type Lock = crate::mutex::simple::RawLock;

#[cfg(not(feature = "parking_lot_core"))]
type Lock = crate::mutex::spin::RawLock;

type ReLock = crate::reentrant::simple::RawReentrantLock<Lock>;

macro_rules! new {
    () => {
        unsafe { ReLock::from_raw_parts(Lock::new(), super::std_thread::StdThreadInfo) }
    };
}

// 61 because it is a large prime number,
// this will reduce contention between unrelated locks
// because unrealated locks will be unlikely to pick up the same lock,
// even they are contigious in memory
#[rustfmt::skip]
static GLOBAL: [ReLock; 61] = [
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(), new!(), new!(), new!(), 
    new!(),
];

unsafe impl crate::reentrant::RawReentrantMutex for Global {}
unsafe impl RawLockInfo for Global {
    const INIT: Self = Self;

    type ExclusiveGuardTraits = <ReLock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <ReLock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawShareLock for Global {
    #[inline]
    fn shr_lock(&self) {
        GLOBAL[self.addr()].shr_lock()
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        GLOBAL[self.addr()].shr_try_lock()
    }

    #[inline]
    unsafe fn shr_split(&self) {
        GLOBAL[self.addr()].shr_split()
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        GLOBAL[self.addr()].shr_unlock()
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        GLOBAL[self.addr()].shr_bump()
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
