use std::cell::UnsafeCell;

use crate::share_lock::{RawShareLockExt, ShareGuard};
use crate::unique_lock::{RawUniqueLockExt, UniqueGuard};

// pub mod simple;
#[cfg(feature = "extra")]
pub mod local_simple;

#[cfg(feature = "extra")]
pub mod local_splittable;

#[repr(C)]
pub struct RwLock<L, T: ?Sized> {
    lock: L,
    value: UnsafeCell<T>,
}

impl<L: crate::RawLockInfo, T: Default> Default for RwLock<L, T> {
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
    pub const unsafe fn from_raw_parts(lock: L, value: T) -> Self {
        Self {
            lock,
            value: UnsafeCell::new(value),
        }
    }

    pub fn into_raw_parts(self) -> (L, T) {
        (self.lock, self.value.into_inner())
    }

    pub fn into_rwlock(self) -> crate::rwlock::RwLock<L, T> {
        let (lock, value) = self.into_raw_parts();
        unsafe { crate::rwlock::RwLock::from_raw_parts(lock, value) }
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, T: ?Sized> RwLock<L, T> {
    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn as_mutex(&self) -> &crate::mutex::Mutex<L, T> {
        unsafe { std::mem::transmute(self) }
    }

    #[allow(clippy::transmute_ptr_to_ptr)]
    pub fn as_mutex_mut(&mut self) -> &mut crate::mutex::Mutex<L, T> {
        unsafe { std::mem::transmute(self) }
    }

    pub unsafe fn raw(&self) -> &L {
        &self.lock
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

impl<L: crate::RawLockInfo, T> RwLock<L, T> {
    pub fn new(value: T) -> Self {
        unsafe { Self::from_raw_parts(L::INIT, value) }
    }
}

impl<L: RawUniqueLockExt, T: ?Sized> RwLock<L, T> {
    pub fn write(&self) -> UniqueGuard<'_, L, T> {
        UniqueGuard::new(self.lock.raw_uniq_lock(), unsafe { &mut *self.value.get() })
    }

    pub fn try_write(&self) -> Option<UniqueGuard<'_, L, T>> {
        Some(UniqueGuard::new(self.lock.try_raw_uniq_lock()?, unsafe {
            &mut *self.value.get()
        }))
    }
}

impl<L: RawShareLockExt, T: ?Sized> RwLock<L, T> {
    pub fn read(&self) -> ShareGuard<'_, L, T> {
        ShareGuard::new(self.lock.raw_shr_lock(), unsafe { &mut *self.value.get() })
    }

    pub fn try_read(&self) -> Option<ShareGuard<'_, L, T>> {
        Some(ShareGuard::new(self.lock.try_raw_shr_lock()?, unsafe {
            &mut *self.value.get()
        }))
    }
}