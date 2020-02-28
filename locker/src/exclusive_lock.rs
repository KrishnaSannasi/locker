pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::{MappedExclusiveGuard, ExclusiveGuard};
pub use raw::RawExclusiveGuard;

use crate::RawLockInfo;

pub trait RawExclusiveLockExt: RawExclusiveLock + RawLockInfo + Sized {
    fn raw_uniq_lock(&self) -> RawExclusiveGuard<Self>;

    fn try_raw_uniq_lock(&self) -> Option<RawExclusiveGuard<Self>>;
}

impl<L: RawExclusiveLock + RawLockInfo> RawExclusiveLockExt for L
where
    Self::ExclusiveGuardTraits: crate::Inhabitted,
{
    fn raw_uniq_lock(&self) -> RawExclusiveGuard<Self> {
        RawExclusiveGuard::new(self, unsafe { std::mem::zeroed() })
    }

    fn try_raw_uniq_lock(&self) -> Option<RawExclusiveGuard<Self>> {
        RawExclusiveGuard::try_new(self, unsafe { std::mem::zeroed() })
    }
}

/// # Safety
///
/// * `uniq_unlock` must be called before before `uniq_lock`,
/// `uniq_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented)
pub unsafe trait RawExclusiveLock {
    /// uniq locks the lock
    ///
    /// blocks until lock is acquired
    fn uniq_lock(&self);

    /// attempts to uniq lock the lock
    ///
    /// returns true on success
    fn uniq_try_lock(&self) -> bool;

    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_unlock(&self);

    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_bump(&self) {
        self.uniq_unlock();
        self.uniq_lock();
    }
}

/// # Safety
///
/// * `uniq_unlock` must be called `n` times before `uniq_lock`,
/// `uniq_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented), where `n` is the number of times
/// `uniq_lock` and `uniq_split` are called combined
pub unsafe trait SplittableExclusiveLock: RawExclusiveLock {
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_split(&self);
}

/// # Safety
/// 
/// same safety notes about `uniq_unlock` apply to `uniq_unlock_fair`
/// same safety notes about `uniq_bump` apply to `uniq_bump_fair`
pub unsafe trait RawExclusiveLockFair: RawExclusiveLock {
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_unlock_fair(&self);
    
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn uniq_bump_fair(&self) {
        self.uniq_unlock_fair();
        self.uniq_lock();
    }
}

unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for &L {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }

    #[inline(always)]
    unsafe fn uniq_bump(&self) {
        L::uniq_bump(self)
    }
}

unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for &mut L {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }

    #[inline(always)]
    unsafe fn uniq_bump(&self) {
        L::uniq_bump(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Box<L> {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }

    #[inline(always)]
    unsafe fn uniq_bump(&self) {
        L::uniq_bump(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Arc<L> {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }

    #[inline(always)]
    unsafe fn uniq_bump(&self) {
        L::uniq_bump(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Rc<L> {
    #[inline(always)]
    fn uniq_lock(&self) {
        L::uniq_lock(self)
    }

    #[inline(always)]
    fn uniq_try_lock(&self) -> bool {
        L::uniq_try_lock(self)
    }

    #[inline(always)]
    unsafe fn uniq_unlock(&self) {
        L::uniq_unlock(self)
    }

    #[inline(always)]
    unsafe fn uniq_bump(&self) {
        L::uniq_bump(self)
    }
}

unsafe impl<L: ?Sized + SplittableExclusiveLock> SplittableExclusiveLock for &L {
    unsafe fn uniq_split(&self) {
        L::uniq_split(self)
    }
}

unsafe impl<L: ?Sized + SplittableExclusiveLock> SplittableExclusiveLock for &mut L {
    unsafe fn uniq_split(&self) {
        L::uniq_split(self)
    }
}
