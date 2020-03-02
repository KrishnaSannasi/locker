use crate::waker_set::WakerSet;
use locker::share_lock::{RawShareGuard as Inner, RawShareLock, RawShareLockFair};
use locker::RawLockInfo;
use std::mem::ManuallyDrop;

pub struct RawShareGuard<'a, L: RawShareLock + RawLockInfo> {
    inner: ManuallyDrop<Inner<'a, L>>,
    waker_set: &'a WakerSet,
}

impl<L: RawShareLock + RawLockInfo> Drop for RawShareGuard<'_, L> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.inner);
            self.waker_set.notify_any();
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo> RawShareGuard<'a, L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// # Safety
            ///
            /// The share lock must be held
            pub const fn from_raw_parts(inner: Inner<'a, L>, waker_set: &'a WakerSet) -> Self {
                Self { inner: ManuallyDrop::new(inner), waker_set }
            }
        } else {
            /// # Safety
            ///
            /// The share lock must be held
            pub fn from_raw_parts(inner: Inner<'a, L>, waker_set: &'a WakerSet) -> Self {
                Self { inner: ManuallyDrop::new(inner), waker_set }
            }
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo> RawShareGuard<'a, L> {
    pub fn inner(&self) -> &Inner<'a, L> {
        &self.inner
    }

    pub fn into_raw_parts(self) -> (Inner<'a, L>, &'a WakerSet) {
        let mut this = std::mem::ManuallyDrop::new(self);

        (
            unsafe { std::mem::ManuallyDrop::take(&mut this.inner) },
            this.waker_set,
        )
    }
}

impl<L: RawShareLock + RawLockInfo> Clone for RawShareGuard<'_, L> {
    fn clone(&self) -> Self {
        Self::from_raw_parts((*self.inner).clone(), self.waker_set)
    }
}
