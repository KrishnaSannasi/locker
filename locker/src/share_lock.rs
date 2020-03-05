//! Generic shared locks
//!
//! See [`RawShareLock`] for details

mod guard;
mod raw;

pub use guard::{MappedShareGuard, ShareGuard};
pub use raw::{RawShareGuard, _RawShareGuard};

#[cfg(doc)]
use crate::RawLockInfo;

/// A raw sharable lock, this implementation is for any lock that can be locked multiple times
/// for some times slice.
///
/// Some examples include `RwLock`'s reader locks and `RefCell`'s `Ref`, and `ReentrantMutex`'s
/// locks (which can be shared in a single thread).
///
/// # *shr lock*
///
/// Throughout this documentation you may see references to *shr lock*. A *shr lock* represents a single lock
/// resource. This resource prevents any thread from acquiring an [*exc lock*](crate::exclusive_lock::RawExclusiveLock#*exc-lock*),
///
/// One acquires ownership of a *shr lock* by calling [`RawShareLock::shr_lock`], by
/// [`RawShareLock::shr_try_lock`] if it returns true, and finally by calling [`RawShareLock::shr_split`]
///
/// One releases ownership a *shr lock* by calling [`RawShareLock::shr_unlock`] or [`RawShareLockFair::shr_unlock_fair`]
///
/// While a *shr lock* exists, then more *shr lock*s can be acquired, by using any of the methods listed above.
/// But an [*exc lock*](crate::exclusive_lock::RawExclusiveLock#*exc-lock*) cannot be acquired.
///
/// A the owner of a *shr lock* must repsect the trait bounds specified by [`RawLockInfo::ShareGuardTraits`].
/// This means that if [`RawLockInfo::ShareGuardTraits`] is not [`Send`], then the *shr lock* cannot be transferred across
/// thread boundries, and if it isn't [`Sync`], then the *shr lock* cannot be shared across thread boundries
///
/// All of these rules are enforced in a safe way through [`RawShareGuard`].
///
/// # Safety
///
/// * `shr_unlock` must be called `n` times before `exc_lock`,
/// `exc_try_lock` can succeed (provided that `RawExclusiveLock` is implemented),
/// where `n` is the number of times `shr_lock` and `shr_split` are called combined
pub unsafe trait RawShareLock {
    /// acquire a *shr locks*
    ///
    /// blocks until lock is acquired
    ///
    /// # Panic
    ///
    /// This function may panic if the lock is cannot be acquired
    fn shr_lock(&self);

    /// attempts to acquire a *shr lock*
    ///
    /// This function is non-blocking and may not panic
    ///
    /// returns true on success
    fn shr_try_lock(&self) -> bool;

    /// Re-acquire the lock without checking if it was already acquired.
    /// This is equivilent to just calling `shr_lock`, but can be more efficient
    /// in most cases.
    ///
    /// acquires a *shr lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_split(&self);

    /// Unlock a single shared lock
    ///
    /// This releases a *shr lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_unlock(&self);

    /// Temporarily yields the lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `shr_unlock` followed by `shr_lock`,
    /// however it can be much more efficient in the case where there are no waiting threads.
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_bump(&self) {
        self.shr_unlock();
        self.shr_lock();
    }
}

/// Additional methods for `RawShareLock` which support locking with timeouts.
///
/// The `Duration` and `Instant` types are specified as associated types so that
/// this trait is usable even in no_std environments.
pub unsafe trait RawShareLockTimed: RawShareLock + crate::RawTimedLock {
    /// attempts to acquire a *shr lock* until a timeout is reached.
    ///
    /// This function is blocking until either the shr lock is acquired
    /// in which case it returns true, or it times out, in which case it
    /// returns false
    ///
    /// returns true on success
    fn shr_try_lock_until(&self, instant: Self::Instant) -> bool;

    /// attempts to acquire a *shr lock* until a timeout is reached.
    ///
    /// This function is blocking until either the shr lock is acquired
    /// in which case it returns true, or it times out, in which case it
    /// returns false
    ///
    /// returns true on success
    fn shr_try_lock_for(&self, duration: Self::Duration) -> bool;
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
/// same safety notes about `shr_unlock` apply to `shr_unlock_fair`
/// same safety notes about `shr_bump` apply to `shr_bump_fair`
pub unsafe trait RawShareLockFair: RawShareLock {
    /// Unlock a single shared lock using a fair unlock protocol
    ///
    /// releases a *shr lock*
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_unlock_fair(&self);

    /// Temporarily yields the lock to a waiting thread if there is one.
    ///
    /// This method is functionally equivalent to calling `shr_unlock_fair` followed by `shr_lock`,
    /// however it can be much more efficient in the case where there are no waiting threads.
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn shr_bump_fair(&self) {
        self.shr_unlock_fair();
        self.shr_lock();
    }
}

/// Additional methods for RwLocks which support atomically downgrading an exclusive lock to a shared lock.
///
/// # Safety
///
/// [`RawShareLockUpgrade::upgrade`] must release a *shr lock* and acquire a *exc lock*
///
/// [`RawShareLockUpgrade::try_upgrade`] must release a *shr lock* and acquire a *exc lock* if it return true
pub unsafe trait RawShareLockUpgrade:
    RawShareLock + crate::exclusive_lock::RawExclusiveLock
{
    /// Atomically upgrade a *shr lock* to a *exc lock*
    ///
    /// Blocks until lock is acquired. This is equivalent to `shr_unlock` followed by `exc_lock`.
    /// But this can be more efficient in the case of no other readers.
    ///
    /// This releases a *shr lock* and acquires a *exc lock*
    ///
    /// # Panic
    ///
    /// This function may panic if the lock is impossible to acquire
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn upgrade(&self);

    /// Attempts to atomically upgrade a *shr lock* to a *exc lock*
    ///
    /// If the *exc lock* was acquired, then the *shr lock* is released
    /// and this function returns true. Otherwise, the *shr lock* is maintained
    /// and this function returns false.
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn try_upgrade(&self) -> bool;
}

/// Additional methods for RwLocks which support atomically downgrading an exclusive lock to a shared lock.
///
/// # Safety
///
/// [`RawShareLockUpgrade::upgrade`] must release a *shr lock* and acquire a *exc lock*
///
/// [`RawShareLockUpgrade::try_upgrade`] must release a *shr lock* and acquire a *exc lock* if it return true
pub unsafe trait RawShareLockUpgradeTimed: RawShareLockUpgrade + RawShareLockTimed {
    /// Attempts to atomically upgrade a *shr lock* to a *exc lock* until a timeout is reached.
    ///
    /// This function is blocking until either the *exc lock* was acquired,
    /// then the *shr lock* is released and this function returns true.
    /// Or the timeout was reached, in which case, the *shr lock* is maintained
    /// and this function returns false.
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn try_upgrade_until(&self, instant: Self::Instant) -> bool;

    /// Attempts to atomically upgrade a *shr lock* to a *exc lock* until a timeout is reached.
    ///
    /// This function is blocking until either the *exc lock* was acquired,
    /// then the *shr lock* is released and this function returns true.
    /// Or the timeout was reached, in which case, the *shr lock* is maintained
    /// and this function returns false.
    ///
    /// # Safety
    ///
    /// * the caller must own a *shr lock*
    /// * the lock must not have been moved since it was locked
    unsafe fn try_upgrade_for(&self, duration: Self::Duration) -> bool;
}

// unsafe impl<L: ?Sized + RawShareLock> RawShareLock for &L {
//     #[inline(always)]
//     fn shr_lock(&self) {
//         L::shr_lock(self)
//     }

//     #[inline(always)]
//     fn shr_try_lock(&self) -> bool {
//         L::shr_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_split(&self) {
//         L::shr_split(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_unlock(&self) {
//         L::shr_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump(&self) {
//         L::shr_bump(self)
//     }
// }

// unsafe impl<L: ?Sized + RawShareLock> RawShareLock for &mut L {
//     #[inline(always)]
//     fn shr_lock(&self) {
//         L::shr_lock(self)
//     }

//     #[inline(always)]
//     fn shr_try_lock(&self) -> bool {
//         L::shr_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_split(&self) {
//         L::shr_split(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_unlock(&self) {
//         L::shr_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump(&self) {
//         L::shr_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Box<L> {
//     #[inline(always)]
//     fn shr_lock(&self) {
//         L::shr_lock(self)
//     }

//     #[inline(always)]
//     fn shr_try_lock(&self) -> bool {
//         L::shr_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_split(&self) {
//         L::shr_split(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_unlock(&self) {
//         L::shr_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump(&self) {
//         L::shr_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Arc<L> {
//     #[inline(always)]
//     fn shr_lock(&self) {
//         L::shr_lock(self)
//     }

//     #[inline(always)]
//     fn shr_try_lock(&self) -> bool {
//         L::shr_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_split(&self) {
//         L::shr_split(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_unlock(&self) {
//         L::shr_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump(&self) {
//         L::shr_bump(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawShareLock> RawShareLock for crate::alloc_prelude::Rc<L> {
//     #[inline(always)]
//     fn shr_lock(&self) {
//         L::shr_lock(self)
//     }

//     #[inline(always)]
//     fn shr_try_lock(&self) -> bool {
//         L::shr_try_lock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_split(&self) {
//         L::shr_split(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_unlock(&self) {
//         L::shr_unlock(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump(&self) {
//         L::shr_bump(self)
//     }
// }

// unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for &L {
//     #[inline(always)]
//     unsafe fn shr_unlock_fair(&self) {
//         L::shr_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump_fair(&self) {
//         L::shr_bump_fair(self)
//     }
// }

// unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for &mut L {
//     #[inline(always)]
//     unsafe fn shr_unlock_fair(&self) {
//         L::shr_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump_fair(&self) {
//         L::shr_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for crate::alloc_prelude::Box<L> {
//     #[inline(always)]
//     unsafe fn shr_unlock_fair(&self) {
//         L::shr_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump_fair(&self) {
//         L::shr_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for crate::alloc_prelude::Arc<L> {
//     #[inline(always)]
//     unsafe fn shr_unlock_fair(&self) {
//         L::shr_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump_fair(&self) {
//         L::shr_bump_fair(self)
//     }
// }

// #[cfg(any(feature = "std", feature = "alloc"))]
// unsafe impl<L: ?Sized + RawShareLockFair> RawShareLockFair for crate::alloc_prelude::Rc<L> {
//     #[inline(always)]
//     unsafe fn shr_unlock_fair(&self) {
//         L::shr_unlock_fair(self)
//     }

//     #[inline(always)]
//     unsafe fn shr_bump_fair(&self) {
//         L::shr_bump_fair(self)
//     }
// }
