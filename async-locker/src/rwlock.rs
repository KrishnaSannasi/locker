use std::cell::UnsafeCell;

use crate::exclusive_lock::ExclusiveGuard;
use crate::share_lock::ShareGuard;
use crate::WakerSet;
use locker::rwlock::RawRwLock;

mod raw;

#[repr(C)]
pub struct RwLock<L, W, T: ?Sized> {
    raw: raw::RwLock<L, W>,
    value: UnsafeCell<T>,
}

impl<L: RawRwLock + locker::Init, W: WakerSet + locker::Init, T: Default> Default
    for RwLock<L, W, T>
{
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Send + RawRwLock, W: Send, T: Send> Send for RwLock<L, W, T> {}
unsafe impl<L: Sync + RawRwLock, W: Sync, T: Send + Sync> Sync for RwLock<L, W, T> {}

impl<L, W, T> RwLock<L, W, T> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::RwLock<L, W>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::RwLock<L, W>, T) {
        (self.raw, self.value.into_inner())
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, W, T: ?Sized> RwLock<L, W, T> {
    #[inline]
    pub const fn raw(&self) -> &raw::RwLock<L, W> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::RwLock<L, W> {
                &mut self.raw
            }

            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::RwLock<L, W> {
                &mut self.raw
            }

            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawRwLock + locker::Init, W: WakerSet + locker::Init, T> RwLock<L, W, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(value: T) -> Self {
                unsafe { Self::from_raw_parts(locker::Init::INIT, value) }
            }
        } else {
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(locker::Init::INIT, value)
            }
        }
    }
}

impl<L: RawRwLock, W: WakerSet, T: ?Sized> RwLock<L, W, T>
where
    L::ExclusiveGuardTraits: locker::marker::Inhabitted,
    L::ShareGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn write(&self) -> ExclusiveGuard<'_, L, W, T> {
        unsafe { ExclusiveGuard::from_raw_parts(self.raw.write().await, self.value.get()) }
    }

    #[inline]
    pub fn try_write(&self) -> Option<ExclusiveGuard<'_, L, W, T>> {
        unsafe {
            Some(ExclusiveGuard::from_raw_parts(
                self.raw.try_write()?,
                self.value.get(),
            ))
        }
    }

    #[inline]
    pub async fn read(&self) -> ShareGuard<'_, L, W, T> {
        unsafe { ShareGuard::from_raw_parts(self.raw.read().await, self.value.get()) }
    }

    #[inline]
    pub fn try_read(&self) -> Option<ShareGuard<'_, L, W, T>> {
        unsafe {
            Some(ShareGuard::from_raw_parts(
                self.raw.try_read()?,
                self.value.get(),
            ))
        }
    }
}
