//! A type-safe implementation of a `Mutex`

use core::cell::UnsafeCell;

use crate::exclusive_lock::{ExclusiveGuard, RawExclusiveLock, RawExclusiveLockTimed};

cfg_if::cfg_if! {
    if #[cfg(feature = "extra")] {
        pub mod global;
        pub mod spin;
        pub mod tagged_spin;
        pub mod local;
        pub mod local_tagged;
        pub mod local_splittable;
        pub mod default;
        pub mod tagged_default;
        pub mod splittable_spin;
        pub mod splittable_default;

        #[cfg(feature = "parking_lot_core")]
        pub mod adaptive;
        #[cfg(feature = "parking_lot_core")]
        pub mod tagged;
        #[cfg(feature = "parking_lot_core")]
        pub mod splittable;
    }
}

pub mod raw;

/// Types implementing this trait can be used by [`Mutex`] to form a safe and fully-functioning mutex type.
///
/// # Safety
///
/// If `Self: Init`, then it must be safe to use `INIT` as the initial value for the lock
pub unsafe trait RawMutex: crate::RawLockInfo + RawExclusiveLock {}

/// A mutual exclusion primitive useful for protecting shared data
///
/// This mutex will block threads waiting for the lock to become available.
/// The mutex can also be statically initialized or created via a `new` constructor (via `nightly` feature flag)
/// or with the `from_raw_parts` (even on `stable`, but `unsafe`).
/// Each mutex has a type parameter which represents the data that it is protecting.
/// The data can only be accessed through the RAII guards returned from `lock` and
/// `try_lock`, which guarantees that the data is only ever accessed when the mutex is locked.
#[repr(C)]
pub struct Mutex<L, T: ?Sized> {
    raw: raw::Mutex<L>,
    value: UnsafeCell<T>,
}

impl<L: RawMutex + crate::Init, T: Default> Default for Mutex<L, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Send + RawMutex, T: Send> Send for Mutex<L, T> {}
unsafe impl<L: Sync + RawMutex, T: Send> Sync for Mutex<L, T> {}

impl<L, T> Mutex<L, T> {
    /// Create a new mutex with the given raw mutex
    #[inline]
    pub const fn from_raw_parts(raw: raw::Mutex<L>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    /// Decomposes the mutex into a raw mutex and it's value
    #[inline]
    pub fn into_raw_parts(self) -> (raw::Mutex<L>, T) {
        (self.raw, self.value.into_inner())
    }

    /// Consumes this mutex, returning the underlying data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> Mutex<L, T> {
    /// the underlying raw mutex
    #[inline]
    pub const fn raw(&self) -> &raw::Mutex<L> {
        &self.raw
    }

    /// Get a raw pointer to the value
    pub fn as_mut_ptr(&self) -> *mut T {
        self.value.get()
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// the underlying raw mutex
            ///
            /// # Safety
            ///
            /// You must not overwrite this raw mutex
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::Mutex<L> {
                &mut self.raw
            }

            /// Returns a mutable reference to the underlying data.
            ///
            /// Since this call borrows the `Mutex` mutably, no actual locking needs to take place
            /// ---the mutable borrow statically guarantees no locks exist.
            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            /// the underlying raw mutex
            ///
            /// # Safety
            ///
            /// You must not overwrite this raw mutex
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::Mutex<L> {
                &mut self.raw
            }

            /// Returns a mutable reference to the underlying data.
            ///
            /// Since this call borrows the `Mutex` mutably, no actual locking needs to take place
            /// ---the mutable borrow statically guarantees no locks exist.
            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawMutex + crate::Init, T> Mutex<L, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Creates a new mutex in an unlocked state ready for use.
            #[inline]
            pub const fn new(value: T) -> Self {
                Self::from_raw_parts(crate::Init::INIT, value)
            }
        } else {
            /// Creates a new mutex in an unlocked state ready for use.
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(crate::Init::INIT, value)
            }
        }
    }
}

impl<L: RawMutex, T: ?Sized> Mutex<L, T>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
{
    #[inline]
    fn wrap<'s>(
        &'s self,
        raw: crate::exclusive_lock::RawExclusiveGuard<'s, L>,
    ) -> ExclusiveGuard<'s, L, T> {
        assert!(core::ptr::eq(self.raw.inner(), raw.inner()));
        unsafe { ExclusiveGuard::from_raw_parts(raw, self.value.get()) }
    }

    /// Acquires a mutex, blocking the current thread until it is able to do so.
    ///
    /// This function will block the current thread until it is available to acquire
    /// the mutex. Upon returning, the thread is the only thread with the mutex held.
    /// An RAII guard is returned to allow scoped unlock of the lock. When the guard
    /// goes out of scope, the mutex will be unlocked.
    ///
    /// Attempts to lock a mutex in the thread which already holds the lock will result in a deadlock or panic.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock or
    /// single threaded mutex)
    #[inline]
    pub fn lock(&self) -> ExclusiveGuard<'_, L, T> {
        self.wrap(self.raw.lock())
    }

    /// Attempts to acquire this lock.
    ///
    /// If the lock could not be acquired at this time, then None is returned.
    /// Otherwise, an RAII guard is returned. The lock will be unlocked when the guard is dropped.
    ///
    /// This function does not block.
    #[inline]
    pub fn try_lock(&self) -> Option<ExclusiveGuard<'_, L, T>> {
        Some(self.wrap(self.raw.try_lock()?))
    }
}

impl<L: RawMutex + RawExclusiveLockTimed, T: ?Sized> Mutex<L, T>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
{
    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    #[inline]
    pub fn try_lock_until(&self, instant: L::Instant) -> Option<ExclusiveGuard<'_, L, T>> {
        Some(self.wrap(self.raw.try_lock_until(instant)?))
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    #[inline]
    pub fn try_lock_for(&self, duration: L::Duration) -> Option<ExclusiveGuard<'_, L, T>> {
        Some(self.wrap(self.raw.try_lock_for(duration)?))
    }
}

unsafe impl<L: ?Sized + RawMutex> RawMutex for &L {}
unsafe impl<L: ?Sized + RawMutex> RawMutex for &mut L {}

#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawMutex> RawMutex for std::boxed::Box<L> {}
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawMutex> RawMutex for std::rc::Rc<L> {}
#[cfg(any(feature = "std", feature = "alloc"))]
unsafe impl<L: ?Sized + RawMutex> RawMutex for std::sync::Arc<L> {}
