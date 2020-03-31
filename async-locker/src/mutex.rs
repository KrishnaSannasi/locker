use std::cell::UnsafeCell;

use crate::exclusive_lock::ExclusiveGuard;
use crate::WakerSet;
use locker::mutex::RawMutex;

pub mod raw;

#[repr(C)]
pub struct Mutex<L, W, T: ?Sized> {
    raw: raw::Mutex<L, W>,
    value: UnsafeCell<T>,
}

impl<L: RawMutex + locker::Init, W: WakerSet + locker::Init, T: Default> Default
    for Mutex<L, W, T>
{
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Send + RawMutex, W: Send, T: Send> Send for Mutex<L, W, T> {}
unsafe impl<L: Sync + RawMutex, W: Sync, T: Send> Sync for Mutex<L, W, T> {}

impl<L, W, T> Mutex<L, W, T> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw_parts(raw: raw::Mutex<L, W>, value: T) -> Self {
        Self {
            raw,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::Mutex<L, W>, T) {
        (self.raw, self.value.into_inner())
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, W, T: ?Sized> Mutex<L, W, T> {
    #[inline]
    pub const fn raw(&self) -> &raw::Mutex<L, W> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mut(&mut self) -> &mut raw::Mutex<L, W> {
                &mut self.raw
            }

            #[inline]
            pub const fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        } else {
            #[inline]
            pub unsafe fn raw_mut(&mut self) -> &mut raw::Mutex<L, W> {
                &mut self.raw
            }

            #[inline]
            pub fn get_mut(&mut self) -> &mut T {
                unsafe { &mut *self.value.get() }
            }
        }
    }
}

impl<L: RawMutex + locker::Init, W: WakerSet + locker::Init, T> Mutex<L, W, T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(value: T) -> Self {
                Self::from_raw_parts(locker::Init::INIT, value)
            }
        } else {
            #[inline]
            pub fn new(value: T) -> Self {
                Self::from_raw_parts(locker::Init::INIT, value)
            }
        }
    }
}

impl<L: RawMutex, W: WakerSet, T: ?Sized> Mutex<L, W, T>
where
    L::ExclusiveGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> ExclusiveGuard<'_, L, W, T> {
        unsafe { ExclusiveGuard::from_raw_parts(self.raw.lock().await, self.value.get()) }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ExclusiveGuard<'_, L, W, T>> {
        unsafe {
            Some(ExclusiveGuard::from_raw_parts(
                self.raw.try_lock()?,
                self.value.get(),
            ))
        }
    }
}
