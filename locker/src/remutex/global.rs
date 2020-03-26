//! A global reentrant mutex

use crate::mutex::default::DefaultLock;
use crate::share_lock::{RawShareLock, RawShareLockFair};
use crate::RawLockInfo;

/// A global lock set that uses the a [`ReLock`](crate::remutex::lock::ReLock)
/// supplied with the [default mutex lock](crate::mutex::default)
/// and the std thread info [`StdThreadInfo`](crate::remutex::std_thread::StdThreadInfo)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalLock;

/// a global raw reentrant mutex
pub type RawReentrantMutex = crate::remutex::raw::ReentrantMutex<GlobalLock>;
/// a global reentrant mutex
pub type ReentrantMutex<T> = crate::remutex::ReentrantMutex<GlobalLock, T>;

impl GlobalLock {
    /// Create a new global raw reentrant mutex
    pub const fn raw_remutex() -> RawReentrantMutex {
        unsafe { RawReentrantMutex::from_raw(Self) }
    }

    /// Create a new global reentrant mutex
    pub const fn remutex<T>(value: T) -> ReentrantMutex<T> {
        ReentrantMutex::from_raw_parts(Self::raw_remutex(), value)
    }

    /// Create a new global reentrant mutex
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn remutex_from_mut<T: ?Sized>(value: &mut T) -> &mut ReentrantMutex<T> {
        unsafe { core::mem::transmute(value) }
    }

    /// Create a new global reentrant mutex
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn remutex_from_mut_slice<T>(value: &mut [T]) -> &mut [ReentrantMutex<T>] {
        unsafe { core::mem::transmute(value) }
    }

    /// Transpose a global reentrant mutex containing a slice into a slice of reentrant global mutexes
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn remutex_transpose<T>(value: &mut ReentrantMutex<[T]>) -> &mut [ReentrantMutex<T>] {
        unsafe { core::mem::transmute(value) }
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn addr(&self) -> usize {
        (self as *const _ as usize) % GLOBAL.len()
    }

    #[inline(always)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn get(&self) -> &'static ReLock {
        &GLOBAL[self.addr()]
    }

    /// Checks if two global locks will contend
    #[inline]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn will_contend(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }

    /// Checks if two global reentrant mutexes will contend
    #[inline]
    pub fn will_remutex_contend<T: ?Sized, U: ?Sized>(
        a: &ReentrantMutex<T>,
        b: &ReentrantMutex<U>,
    ) -> bool {
        a.raw().inner().addr() == b.raw().inner().addr()
    }
}

type ReLock = crate::remutex::lock::ReLock<DefaultLock>;

// 61 because it is a large prime number,
// this will reduce contention between unrelated locks
// because unrealated locks will be unlikely to pick up the same lock,
// even they are contigious in memory
#[rustfmt::skip]
static GLOBAL: [ReLock; 61] = [
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT, crate::Init::INIT, crate::Init::INIT, crate::Init::INIT,
    crate::Init::INIT,
];

impl crate::Init for GlobalLock {
    const INIT: Self = Self;
}

unsafe impl crate::remutex::RawReentrantMutex for GlobalLock {}
unsafe impl RawLockInfo for GlobalLock {
    type ExclusiveGuardTraits = <ReLock as RawLockInfo>::ExclusiveGuardTraits;
    type ShareGuardTraits = <ReLock as RawLockInfo>::ShareGuardTraits;
}

unsafe impl RawShareLock for GlobalLock {
    #[inline]
    fn shr_lock(&self) {
        self.get().shr_lock()
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        self.get().shr_try_lock()
    }

    #[inline]
    unsafe fn shr_split(&self) {
        self.get().shr_split()
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.get().shr_unlock()
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        self.get().shr_bump()
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
impl crate::RawTimedLock for GlobalLock {
    type Instant = std::time::Instant;
    type Duration = std::time::Duration;
}

#[cfg(feature = "parking_lot_core")]
unsafe impl crate::share_lock::RawShareLockTimed for GlobalLock {
    #[inline]
    fn shr_try_lock_until(&self, instant: Self::Instant) -> bool {
        self.get().shr_try_lock_until(instant)
    }

    #[inline]
    fn shr_try_lock_for(&self, duration: Self::Duration) -> bool {
        self.get().shr_try_lock_for(duration)
    }
}
