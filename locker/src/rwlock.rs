use std::cell::UnsafeCell;

use crate::exclusive_lock::ExclusiveGuard;
use crate::share_lock::{RawShareLock, ShareGuard};

cfg_if::cfg_if! {
    if #[cfg(feature = "extra")] {
        pub mod global;
        pub mod spin;
        pub mod local_simple;
        pub mod local_splittable;

        #[cfg(feature = "parking_lot_core")]
        pub mod simple;
        #[cfg(feature = "parking_lot_core")]
        pub mod splittable;
    }
}

mod raw;

pub unsafe trait RawRwLock: crate::mutex::RawMutex + RawShareLock {}
#[repr(C)]
pub struct RwLock<L, T: ?Sized> {
    raw: raw::RwLock<L>,
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
    pub const fn from_raw_parts(raw: raw::RwLock<L>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::RwLock<L>, T) {
        (self.raw, self.value.into_inner())
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
    pub const fn raw(&self) -> &raw::RwLock<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::RwLock<L> {
                &mut self.raw
            }

            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::RwLock<L> {
                &mut self.raw
            }

            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawRwLock, T> RwLock<L, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(value: T) -> Self {
                unsafe { Self::from_raw_parts(L::INIT, value) }
            }
        } else {
            #[inline]
            pub fn new(value: T) -> Self {
                unsafe { Self::from_raw_parts(raw::RwLock::from_raw(L::INIT), value) }
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
    pub fn write(&self) -> ExclusiveGuard<'_, L, T> {
        unsafe { ExclusiveGuard::from_raw_parts(self.raw.write(), self.value.get()) }
    }

    #[inline]
    pub fn try_write(&self) -> Option<ExclusiveGuard<'_, L, T>> {
        unsafe {
            Some(ExclusiveGuard::from_raw_parts(
                self.raw.try_write()?,
                self.value.get(),
            ))
        }
    }

    #[inline]
    pub fn read(&self) -> ShareGuard<'_, L, T> {
        unsafe { ShareGuard::from_raw_parts(self.raw.read(), self.value.get()) }
    }

    #[inline]
    pub fn try_read(&self) -> Option<ShareGuard<'_, L, T>> {
        unsafe {
            Some(ShareGuard::from_raw_parts(
                self.raw.try_read()?,
                self.value.get(),
            ))
        }
    }
}
