//! Generic exclusive locks
//!
//! See [`RawExclusiveLock`] for details

#[doc(hidden)]
pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::{ExclusiveGuard, MappedExclusiveGuard};
pub use raw::RawExclusiveGuard;

#[cfg(doc)]
use crate::RawLockInfo;

/// A raw exclusive lock, this implementation is for any lock that can only be locked once
/// for any time slice.
///
/// Some examples include `RwLock`'s writer locks and `RefCell`'s `RefMut`, and
/// `Mutex`'s locks.
///
/// # *exc lock*
///
/// Throughout this documentation you may see references to *shr lock*. A *exc lock* represents a single lock
/// resource. This resource prevents any thread from acquiring another *exc lock*
/// (except by using [`SplittableExclusiveLock::exc_split`]) or acquiring any [*shr lock*](crate::share_lock::RawShareLock#shr-lock)s
///
/// One acquires ownership of a *exc lock* by calling [`RawExclusiveLock::exc_lock`], by
/// [`RawExclusiveLock::exc_try_lock`] if it returns true, and finally by calling [`SplittableExclusiveLock::exc_split`]
///
/// One releases a *exc lock* by calling [`RawExclusiveLock::exc_unlock`] or [`RawExclusiveLockFair::exc_unlock_fair`]
///
/// A the owner of a *exc lock* must repsect the trait bounds specified by [`RawLockInfo::ExclusiveGuardTraits`].
/// This means that if [`RawLockInfo::ExclusiveGuardTraits`] is not [`Send`], then the *exc lock* cannot be transferred across
/// thread boundries, and if it isn't [`Sync`], then the *exc lock* cannot be shared across thread boundries
///
/// All of these rules are enforced in a safe way through [`RawExclusiveGuard`].
///
/// ### `exc_split`
///
/// It is possible to hold multiple *exc lock* resources at the same time, by using [`SplittableExclusiveLock::exc_split`].
/// In this case, each *exc lock* must guard access to completely disjoint resources.
///
/// # Safety
///
/// * `exc_unlock` must be called before before `exc_lock`,
/// `exc_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented)
pub unsafe trait RawExclusiveLock {
    /// acquire an *exc lock*
    ///
    /// blocks until lock is acquired
    ///
    /// # Panic
    ///
    /// This function may panic if the lock is cannot be acquired
    fn exc_lock(&self);

    /// attempts to acquire a *exc lock*
    ///
    /// This function is non-blocking and may not panic
    ///
    /// returns true on success
    fn exc_try_lock(&self) -> bool;

    /// Unlock a single exclusive lock
    ///
    /// This releases a *exc lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_unlock(&self);

    /// Temporarily yields the lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `exc_unlock` followed by `exc_lock`,
    /// however it can be much more efficient in the case where there are no waiting threads.
    ///
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_bump(&self) {
        self.exc_unlock();
        self.exc_lock();
    }
}

/// # Safety
///
/// * `exc_unlock` must be called `n` times before `exc_lock`,
/// `exc_try_lock`, `shr_lock`, or `try_shr_lock` can succeed (for the last two,
/// provided that `RawShareLock` is implemented), where `n` is the number of times
/// `exc_lock` and `exc_split` are called combined
pub unsafe trait SplittableExclusiveLock: RawExclusiveLock {
    /// Re-acquire the lock without checking if it was already acquired.
    /// This can be used to logically split the lock into multiple non-overlapping
    /// parts.
    ///
    /// i.e. [`ExclusiveGuard::split_map`](crate::exclusive_lock::guard::ExclusiveGuard::split_map)
    ///
    /// acquires a *exc lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_split(&self);
}

/// Additional methods for locks which support fair unlocking.
///
/// Fair unlocking means that a lock is handed directly over to
/// the next waiting thread if there is one, without giving other
/// threads the opportunity to "steal" the lock in the meantime.
/// This is typically slower than unfair unlocking, but may be necessary
/// in certain circumstances.
///
/// # Safety
///
/// same safety notes about `exc_unlock` apply to `exc_unlock_fair`
/// same safety notes about `exc_bump` apply to `exc_bump_fair`
pub unsafe trait RawExclusiveLockFair: RawExclusiveLock {
    /// Unlock a single exclusive lock using a fair unlock protocol
    ///
    /// releases a *exc lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_unlock_fair(&self);

    /// Temporarily yields the lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `exc_unlock_fair` followed by `exc_lock`,
    /// however it can be much more efficient in the case where there are no waiting threads.
    ///
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn exc_bump_fair(&self) {
        self.exc_unlock_fair();
        self.exc_lock();
    }
}

/// Additional methods for RwLocks which support atomically downgrading an exclusive lock to a shared lock.
///
/// # Safety
///
/// [`RawExclusiveLockDowngrade::downgrade`] must release a *exc lock* and acquire a *shr lock*, and must not let any other thread
/// acquire a lock in between.
pub unsafe trait RawExclusiveLockDowngrade:
    RawExclusiveLock + crate::share_lock::RawShareLock
{
    /// Atomically downgrades a *exc lock* to a *shr lock*
    ///
    /// This is equivalent to `exc_unlock` followed by `shr_lock`, but it is
    /// non-blocking, and cannot be preempted.
    ///
    /// This releases a *exc lock* and acquires a *shr lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a exclusive lock
    /// * the lock must not have been moved since it was locked
    unsafe fn downgrade(&self);
}

// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for &L {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for &mut L {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Box<L> {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Arc<L> {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLock> RawExclusiveLock for crate::alloc_prelude::Rc<L> {
//     #[inline(always)]
//     fn exc_lock(&self) {
//         L::exc_lock(self)
//     }

//     #[inline(always)]
//     fn exc_try_lock(&self) -> bool {
//         L::exc_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_unlock(&self) {
//         L::exc_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump(&self) {
//         L::exc_bump(self)
//     }
// }

// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for &L {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for &mut L {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair
//     for crate::alloc_prelude::Box<L>
// {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair
//     for crate::alloc_prelude::Arc<L>
// {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawExclusiveLockFair> RawExclusiveLockFair for crate::alloc_prelude::Rc<L> {
//     #[inline(always)]
//     unsafe fn exc_unlock_fair(&self) {
//         L::exc_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn exc_bump_fair(&self) {
//         L::exc_bump_fair(self)
//     }
// }

// unsafe impl<L: ?Sized + SplittableExclusiveLock> SplittableExclusiveLock for &L {
//     unsafe fn exc_split(&self) {
//         L::exc_split(self)
//     }
// }

// unsafe impl<L: ?Sized + SplittableExclusiveLock> SplittableExclusiveLock for &mut L {
//     unsafe fn exc_split(&self) {
//         L::exc_split(self)
//     }
// }
