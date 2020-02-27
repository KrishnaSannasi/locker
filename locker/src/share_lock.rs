pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::{ShareGuard, MappedShareGuard};
pub use raw::RawShareGuard;

use crate::RawLockInfo;

pub trait RawShareLockExt: RawShareLock + RawLockInfo + Sized {
    fn raw_shr_lock(&self) -> RawShareGuard<Self>;

    fn try_raw_shr_lock(&self) -> Option<RawShareGuard<Self>>;
}

impl<L: RawShareLock + RawLockInfo> RawShareLockExt for L
where
    Self::ShareGuardTraits: crate::Inhabitted,
{
    fn raw_shr_lock(&self) -> RawShareGuard<Self> {
        RawShareGuard::new(self, unsafe { std::mem::zeroed() })
    }

    fn try_raw_shr_lock(&self) -> Option<RawShareGuard<Self>> {
        RawShareGuard::try_new(self, unsafe { std::mem::zeroed() })
    }
}

/// # Safety
/// 
/// * `shr_unlock` must be called `n` times before `uniq_lock`,
/// `uniq_try_lock` can succeed (provided that `RawUniqueLock` is implemented), 
/// where `n` is the number of times `shr_lock` and `shr_split` are called combined
pub unsafe trait RawShareLock {
    /// shr locks the lock
    /// 
    /// blocks until lock is acquired
    fn shr_lock(&self);

    /// attempts to shr lock the lock
    /// 
    /// returns true on success
    fn shr_try_lock(&self) -> bool;

    /// # Safety
    ///
    /// * This lock must be share locked before calling this function
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_split(&self);

    /// # Safety
    ///
    /// This Share lock must be locked before calling this function
    unsafe fn shr_unlock(&self);
}

unsafe impl<L: ?Sized + RawShareLock> RawShareLock for &L {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }
}

unsafe impl<L: ?Sized + RawShareLock> RawShareLock for &mut L {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }
}

#[cfg(any(feature="std", feature="alloc"))]
unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Box<L> {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }
}

#[cfg(any(feature="std", feature="alloc"))]
unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Arc<L> {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }
}

#[cfg(any(feature="std", feature="alloc"))]
unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Rc<L> {
    #[inline(always)]
    fn shr_lock(&self) {
        L::shr_lock(self)
    }

    #[inline(always)]
    fn shr_try_lock(&self) -> bool {
        L::shr_try_lock(self)
    }

    #[inline(always)]
    unsafe fn shr_split(&self) {
        L::shr_split(self)
    }

    #[inline(always)]
    unsafe fn shr_unlock(&self) {
        L::shr_unlock(self)
    }
}
