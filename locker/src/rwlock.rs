//! a type safe implementation of a `RwLock`

use std::cell::UnsafeCell;

use crate::exclusive_lock::{ExclusiveGuard, RawExclusiveLockTimed};
use crate::share_lock::{RawShareLock, RawShareLockTimed, ShareGuard};

cfg_if::cfg_if! {
    if #[cfg(feature = "extra")] {
        pub mod global;
        pub mod spin;
        pub mod local;
        pub mod default;
        pub mod local_splittable;
        pub mod splittable_spin;
        pub mod splittable_default;

        #[cfg(feature = "parking_lot_core")]
        pub mod adaptive;
        #[cfg(feature = "parking_lot_core")]
        pub mod splittable;
    }
}

pub mod raw;

/// Types implementing this trait can be used by [`RwLock`] to form a safe and fully-functioning rwlock type.
///
/// # Safety
///
/// A *shr lock* cannot exist at the same time as a *exc lock*
///
/// If `Self: Init`, then it must be safe to use `INIT` as the initial value for the lock
pub unsafe trait RawRwLock: crate::mutex::RawMutex + RawShareLock {}

/// A read-write syncronization primitive useful for protecting shared data
///
/// This rwlock will block threads waiting for the lock to become available.
/// The rwlock can also be statically initialized or created via a `from_raw_parts` constructor.
#[repr(C)]
pub struct RwLock<L, T: ?Sized> {
    raw: raw::RwLock<L>,
    value: UnsafeCell<T>,
}

impl<L: RawRwLock + crate::Init, T: Default> Default for RwLock<L, T> {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

unsafe impl<L: Send, T: Send> Send for RwLock<L, T> {}
unsafe impl<L: Sync, T: Send + Sync> Sync for RwLock<L, T> {}

impl<L, T> RwLock<L, T> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::RwLock<L>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    /// Decomposes the rwlock into a raw rwlock and it's value
    #[inline]
    pub fn into_raw_parts(self) -> (raw::RwLock<L>, T) {
        (self.raw, self.value.into_inner())
    }

    /// Consumes this rwlock, returning the underlying data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> RwLock<L, T> {
    /// the underlying raw rwlock
    #[inline]
    pub const fn raw(&self) -> &raw::RwLock<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// the underlying raw rwlock
            ///
            /// # Safety
            ///
            /// You must not overwrite this raw rwlock
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::RwLock<L> {
                &mut self.raw
            }

            /// Returns a mutable reference to the underlying data.
            ///
            /// Since this call borrows the `RwLock` mutably, no actual locking needs to take place
            /// ---the mutable borrow statically guarantees no locks exist.
            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            /// the underlying raw rwlock
            ///
            /// # Safety
            ///
            /// You must not overwrite this raw rwlock
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::RwLock<L> {
                &mut self.raw
            }

            /// Returns a mutable reference to the underlying data.
            ///
            /// Since this call borrows the `RwLock` mutably, no actual locking needs to take place
            /// ---the mutable borrow statically guarantees no locks exist.
            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawRwLock + crate::Init, T> RwLock<L, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// Creates a new rwlock in an unlocked state ready for use.
            #[inline]
            pub const fn new(value: T) -> Self {
                Self::from_raw_parts(crate::Init::INIT, value)
            }
        } else {
            /// Creates a new rwlock in an unlocked state ready for use.
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(crate::Init::INIT, value)
            }
        }
    }
}

impl<L: RawRwLock, T: ?Sized> RwLock<L, T>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
    L::ShareGuardTraits: crate::Inhabitted,
{
    #[inline]
    fn wrap_write<'s>(
        &'s self,
        raw: crate::exclusive_lock::RawExclusiveGuard<'s, L>,
    ) -> ExclusiveGuard<'s, L, T> {
        assert!(std::ptr::eq(self.raw.inner(), raw.inner()));
        unsafe { ExclusiveGuard::from_raw_parts(raw, self.value.get()) }
    }

    #[inline]
    fn wrap_read<'s>(
        &'s self,
        raw: crate::share_lock::RawShareGuard<'s, L>,
    ) -> ShareGuard<'s, L, T> {
        assert!(std::ptr::eq(self.raw.inner(), raw.inner()));
        unsafe { ShareGuard::from_raw_parts(raw, self.value.get()) }
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock` when dropped.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock or
    /// single threaded rwlock)
    #[inline]
    pub fn write(&self) -> ExclusiveGuard<'_, L, T> {
        self.wrap_write(self.raw.write())
    }

    /// Attempts to lock this `RwLock` with exclusive write access.
    ///
    /// If the lock could not be acquired at this time, then None is returned.
    /// Otherwise, an RAII guard is returned which will release the lock when it is dropped.
    ///
    /// This function does not block or panic.
    #[inline]
    pub fn try_write(&self) -> Option<ExclusiveGuard<'_, L, T>> {
        Some(self.wrap_write(self.raw.try_write()?))
    }

    /// Locks this `RwLock` with shared read access, blocking the current thread until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which hold the lock.
    /// There may be other readers currently inside the lock when this method returns.
    ///
    /// Note that attempts to recursively acquire a read lock on a `RwLock` when the current thread
    /// already holds one may result in a deadlock/panic.
    ///
    /// Returns an RAII guard which will release this thread's shared access once it is dropped.
    ///
    /// # Panic
    ///
    /// This function may panic if it is impossible to acquire the lock (in the case of deadlock or
    /// single threaded rwlock)
    #[inline]
    pub fn read(&self) -> ShareGuard<'_, L, T> {
        self.wrap_read(self.raw.read())
    }

    /// Attempts to acquire this `RwLock` with shared read access.
    ///
    /// If the access could not be granted at this time, then None is returned.
    /// Otherwise, an RAII guard is returned which will release the shared access when it is dropped.
    ///
    /// This function does not block or panic.
    #[inline]
    pub fn try_read(&self) -> Option<ShareGuard<'_, L, T>> {
        Some(self.wrap_read(self.raw.try_read()?))
    }
}

impl<L: RawRwLock + RawExclusiveLockTimed + RawShareLockTimed, T: ?Sized> RwLock<L, T>
where
    L::ExclusiveGuardTraits: crate::Inhabitted,
    L::ShareGuardTraits: crate::Inhabitted,
{
    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    #[inline]
    pub fn try_write_until(&self, instant: L::Instant) -> Option<ExclusiveGuard<'_, L, T>> {
        Some(self.wrap_write(self.raw.try_write_until(instant)?))
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    #[inline]
    pub fn try_write_for(&self, duration: L::Duration) -> Option<ExclusiveGuard<'_, L, T>> {
        Some(self.wrap_write(self.raw.try_write_for(duration)?))
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    #[inline]
    pub fn try_read_until(&self, instant: L::Instant) -> Option<ShareGuard<'_, L, T>> {
        Some(self.wrap_read(self.raw.try_read_until(instant)?))
    }

    /// Attempts to acquire this lock until a timeout is reached.
    ///
    /// If the lock could not be acquired before the timeout expired,
    /// then None is returned. Otherwise, an RAII guard is returned.
    /// The lock will be unlocked when the guard is dropped.
    #[inline]
    pub fn try_read_for(&self, duration: L::Duration) -> Option<ShareGuard<'_, L, T>> {
        Some(self.wrap_read(self.raw.try_read_for(duration)?))
    }
}
