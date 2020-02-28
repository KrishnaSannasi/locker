use std::cell::UnsafeCell;

use crate::share_lock::{RawShareLock, RawShareLockExt, ShareGuard};
use crate::exclusive_lock::ExclusiveGuard;

#[cfg(feature = "extra")]
pub mod global;

#[cfg(feature = "parking_lot_core")]
pub mod simple;

#[cfg(feature = "extra")]
pub mod spin;

#[cfg(feature = "extra")]
pub mod local_simple;

#[cfg(feature = "extra")]
pub mod local_splittable;

pub unsafe trait RawRwLock: crate::mutex::RawMutex + RawShareLock + RawShareLockExt {}
#[repr(C)]
pub struct RwLock<L, T: ?Sized> {
    lock: L,
    value: UnsafeCell<T>,
}

impl<L: RawRwLock, T: Default> Default for RwLock<L, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Send, T: Send> Send for RwLock<L, T> {}
unsafe impl<L: Sync, T: Send + Sync> Sync for RwLock<L, T> {}

impl<L, T> RwLock<L, T> {
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
    pub fn into_rwlock(self) -> crate::rwlock::RwLock<L, T> {
        let (lock, value) = self.into_raw_parts();
        unsafe { crate::rwlock::RwLock::from_raw_parts(lock, value) }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> RwLock<L, T> {
    #[inline]
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn as_mutex(&self) -> &crate::mutex::Mutex<L, T> {
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn as_mutex_mut(&mut self) -> &mut crate::mutex::Mutex<L, T> {
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

impl<L: RawRwLock, T> RwLock<L, T> {
    #[inline]
    pub fn new(value: T) -> Self {
        unsafe { Self::from_raw_parts(L::INIT, value) }
    }
}

impl<L: RawRwLock, T: ?Sized> RwLock<L, T> {
    #[inline]
    pub fn write(&self) -> ExclusiveGuard<'_, L, T> {
        unsafe {
            ExclusiveGuard::from_raw_parts(
                self.lock.raw_uniq_lock(), 
                &mut *self.value.get(),
            )
        }
    }

    #[inline]
    pub fn try_write(&self) -> Option<ExclusiveGuard<'_, L, T>> {
        unsafe {
            Some(ExclusiveGuard::from_raw_parts(
                self.lock.try_raw_uniq_lock()?,
                &mut *self.value.get(),
            ))
        }
    }

    #[inline]
    pub fn read(&self) -> ShareGuard<'_, L, T> {
        unsafe {
            ShareGuard::from_raw_parts(
                self.lock.raw_shr_lock(),
                &*self.value.get(),
            )
        }
    }

    #[inline]
    pub fn try_read(&self) -> Option<ShareGuard<'_, L, T>> {
        unsafe {
            Some(ShareGuard::from_raw_parts(
                self.lock.try_raw_shr_lock()?,
                &*self.value.get(),
            ))
        }
    }
}
