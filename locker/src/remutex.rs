//! a reentrant mutex

use core::cell::UnsafeCell;
use core::num::NonZeroUsize;

use crate::share_lock::{RawShareLock, RawShareLockTimed, ShareGuard};

#[cfg(feature = "extra")]
pub mod lock;

#[cfg(feature = "extra")]
pub mod counter;

#[cfg(feature = "std")]
pub mod std_thread;

#[cfg(all(feature = "extra", feature = "std"))]
pub mod global;

pub mod raw;

/// Get the current thread id
///
/// # Safety
///
/// Implementations of this trait must ensure that no two active threads share
/// the same thread ID. However the ID of a thread that has exited can be re-used
/// since that thread is no longer active.
pub unsafe trait ThreadInfo {
    /// The id of the current thread
    fn id(&self) -> NonZeroUsize;
}

/// Types implementing this trait can be used by [`ReentrantMutex`] to
/// form a safe and fully-functioning reentrant mutex type.
///
/// # Safety
///
/// A *shr lock*'s cannot be shared across multiple threads. i.e. two distinct threads can't
/// own a *shr lock* at the same time.
pub unsafe trait RawReentrantMutex: crate::RawLockInfo + RawShareLock {}

/// A mutual exclusion primitive useful for protecting shared data
///
/// This reentrant mutex will block threads waiting for the lock to become available.
/// The reentrant mutex can also be statically initialized or created via a `new` constructor (via `nightly` feature flag)
/// or with the `from_raw_parts` (even on `stable`, but `unsafe`).
/// Each reentrant mutex has a type parameter which represents the data that it is protecting.
/// The data can only be accessed through the RAII guards returned from `lock` and
/// `try_lock`, which guarantees that the data is only ever accessed when the mutex is locked.
///
/// Each lock can only be used in a single thread, and the reentrant mutex can be locked as many times
/// within a single thread. But two threads can't both hold a lock into the reentrant mutex as the same time.
#[repr(C)]
pub struct ReentrantMutex<L, T: ?Sized> {
    raw: raw::ReentrantMutex<L>,
    value: UnsafeCell<T>,
}

impl<L: RawReentrantMutex + crate::Init, T: Default> Default for ReentrantMutex<L, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Sync + RawReentrantMutex, T: Send> Sync for ReentrantMutex<L, T> {}

impl<L, T> ReentrantMutex<L, T> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::ReentrantMutex<L>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    /// Decomposes the mutex into a raw mutex and it's value
    #[inline]
    pub fn into_raw_parts(self) -> (raw::ReentrantMutex<L>, T) {
        (self.raw, self.value.into_inner())
    }

    /// Consumes this mutex, returning the underlying data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> ReentrantMutex<L, T> {
    /// the underlying raw reentrant mutex
    #[inline]
    pub const fn raw(&self) -> &raw::ReentrantMutex<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// # Safety
            ///
            /// You must not overwrite the raw lock
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::ReentrantMutex<L> {
                &mut self.raw
            }

            /// Returns a mutable reference to the underlying data.
            ///
            /// Since this call borrows the `ReentrantMutex` mutably, no actual locking needs to take place
            /// ---the mutable borrow statically guarantees no locks exist.
            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            /// # Safety
            ///
            /// You must not overwrite the raw lock
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::ReentrantMutex<L> {
                &mut self.raw
            }

            /// Returns a mutable reference to the underlying data.
            ///
            /// Since this call borrows the `ReentrantMutex` mutably, no actual locking needs to take place
            /// ---the mutable borrow statically guarantees no locks exist.
            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawReentrantMutex + crate::Init, T> ReentrantMutex<L, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Create a new reentrant mutex
            #[inline]
            pub const fn new(value: T) -> Self {
                Self::from_raw_parts(crate::Init::INIT, value)
            }
        } else {
            /// Create a new reentrant mutex
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(crate::Init::INIT, value)
            }
        }
    }
}

impl<L: RawReentrantMutex, T: ?Sized> ReentrantMutex<L, T>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    #[inline]
    fn wrap<'s>(&'s self, raw: crate::share_lock::RawShareGuard<'s, L>) -> ShareGuard<'s, L, T> {
        assert!(core::ptr::eq(self.raw.inner(), raw.inner()));
        unsafe { ShareGuard::from_raw_parts(raw, self.value.get()) }
    }

    /// Acquires a lock, blocking the current thread until it is able to do so.
    ///
    /// This function will block the current thread until it is available to acquire
    /// the mutex. Upon returning, the thread is the only thread with the mutex held.
    /// An RAII guard is returned to allow scoped unlock of the lock. When the guard
    /// goes out of scope, the mutex will be unlocked.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock)
    #[inline]
    pub fn lock(&self) -> ShareGuard<'_, L, T> {
        self.wrap(self.raw.lock())
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then None is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the guard is dropped.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    #[inline]
    pub fn try_lock(&self) -> Option<ShareGuard<'_, L, T>> {
        Some(self.wrap(self.raw.try_lock()?))
    }
}

impl<L: RawReentrantMutex + RawShareLockTimed, T: ?Sized> ReentrantMutex<L, T>
where
    L::ShareGuardTraits: crate::Inhabitted,
{
    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    #[inline]
    pub fn try_lock_until(&self, instant: L::Instant) -> Option<ShareGuard<'_, L, T>> {
        Some(self.wrap(self.raw.try_lock_until(instant)?))
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    ///
    /// If there is already a lock acquired in the current thread, then this function is non-blocking
    /// and is guaranteed to acquire the lock.
    #[inline]
    pub fn try_lock_for(&self, duration: L::Duration) -> Option<ShareGuard<'_, L, T>> {
        Some(self.wrap(self.raw.try_lock_for(duration)?))
    }
}

unsafe impl<L: ?Sized + RawReentrantMutex> RawReentrantMutex for &L {}
unsafe impl<L: ?Sized + RawReentrantMutex> RawReentrantMutex for &mut L {}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawReentrantMutex> RawReentrantMutex for std::boxed::Box<L> {}
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawReentrantMutex> RawReentrantMutex for std::rc::Rc<L> {}
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawReentrantMutex> RawReentrantMutex for std::sync::Arc<L> {}

unsafe impl<L: ?Sized + ThreadInfo> ThreadInfo for &L {
    fn id(&self) -> core::num::NonZeroUsize {
        L::id(self)
    }
}

unsafe impl<L: ?Sized + ThreadInfo> ThreadInfo for &mut L {
    fn id(&self) -> core::num::NonZeroUsize {
        L::id(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + ThreadInfo> ThreadInfo for std::boxed::Box<L> {
    fn id(&self) -> core::num::NonZeroUsize {
        L::id(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + ThreadInfo> ThreadInfo for std::rc::Rc<L> {
    fn id(&self) -> core::num::NonZeroUsize {
        L::id(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + ThreadInfo> ThreadInfo for std::sync::Arc<L> {
    fn id(&self) -> core::num::NonZeroUsize {
        L::id(self)
    }
}
