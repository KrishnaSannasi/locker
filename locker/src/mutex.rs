use std::cell::UnsafeCell;

use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockExt, ExclusiveGuard};

#[cfg(feature = "extra")]
pub mod global;

#[cfg(feature = "extra")]
pub mod spin;

#[cfg(feature = "extra")]
pub mod local_simple;
#[cfg(feature = "parking_lot_core")]
pub mod simple;

#[cfg(feature = "extra")]
pub mod local_splittable;
#[cfg(feature = "parking_lot_core")]
pub mod splittable;

pub mod prelude {
    #[cfg(feature = "parking_lot_core")]
    pub type Mutex<T> = super::Mutex<super::simple::RawLock, T>;
    #[cfg(feature = "extra")]
    pub type LocalMutex<T> = super::Mutex<super::local_simple::RawLock, T>;

    #[cfg(feature = "parking_lot_core")]
    pub type SplittableMutex<T> = super::Mutex<super::splittable::RawLock, T>;
    #[cfg(feature = "extra")]
    pub type LocalSplittableMutex<T> = super::Mutex<super::local_splittable::RawLock, T>;
}

pub unsafe trait RawMutex: crate::RawLockInfo + RawExclusiveLock + RawExclusiveLockExt {}
#[repr(C)]
pub struct Mutex<L, T: ?Sized> {
    lock: L,
    value: UnsafeCell<T>,
}

impl<L: RawMutex, T: Default> Default for Mutex<L, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Send + RawMutex, T: Send> Send for Mutex<L, T> {}
unsafe impl<L: Sync + RawMutex, T: Send> Sync for Mutex<L, T> {}

impl<L, T> Mutex<L, T> {
    /// # Safety
    ///
    /// You must pass `RawUniueLock::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw_parts(lock: L, value: T) -> Self {
        Self {
            lock,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (L, T) {
        (self.lock, self.value.into_inner())
    }

    #[inline]
    pub fn into_mutex(self) -> crate::mutex::Mutex<L, T> {
        let (lock, value) = self.into_raw_parts();
        unsafe { crate::mutex::Mutex::from_raw_parts(lock, value) }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> Mutex<L, T> {
    #[inline]
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn as_rwlock(&self) -> &crate::rwlock::RwLock<L, T> {
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn as_rwlock_mut(&mut self) -> &mut crate::rwlock::RwLock<L, T> {
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub unsafe fn raw(&self) -> &L {
        &self.lock
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

impl<L: RawMutex, T> Mutex<L, T> {
    #[inline]
    pub fn new(value: T) -> Self {
        unsafe { Self::from_raw_parts(L::INIT, value) }
    }
}

impl<L: RawMutex, T: ?Sized> Mutex<L, T> {
    #[inline]
    pub fn lock(&self) -> ExclusiveGuard<'_, L, T> {
        unsafe {
            ExclusiveGuard::from_raw_parts(
                self.lock.raw_uniq_lock(), 
                &mut *self.value.get(),
            )
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ExclusiveGuard<'_, L, T>> {
        unsafe {
            Some(ExclusiveGuard::from_raw_parts(
                self.lock.try_raw_uniq_lock()?,
                &mut *self.value.get(),
            ))
        }
    }
}
