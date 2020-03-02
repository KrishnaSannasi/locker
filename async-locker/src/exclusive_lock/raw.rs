use crate::waker_set::WakerSet;
use locker::exclusive_lock::{
    RawExclusiveGuard as Inner, RawExclusiveLock, RawExclusiveLockDowngrade, RawExclusiveLockFair,
    SplittableExclusiveLock,
};
use locker::RawLockInfo;
use std::mem::ManuallyDrop;

pub struct RawExclusiveGuard<'a, L: RawExclusiveLock + RawLockInfo> {
    inner: ManuallyDrop<Inner<'a, L>>,
    waker_set: &'a WakerSet,
}

impl<L: RawExclusiveLock + RawLockInfo> Drop for RawExclusiveGuard<'_, L> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.inner);
            self.waker_set.notify_any();
        }
    }
}

impl<'a, L: RawExclusiveLock + RawLockInfo> RawExclusiveGuard<'a, L> {
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

impl<'a, L: RawExclusiveLock + RawLockInfo> RawExclusiveGuard<'a, L> {
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

// impl<'a, L: RawExclusiveLockDowngrade + RawLockInfo> RawExclusiveGuard<'a, L>
// where
//     L::ShareGuardTraits: locker::Inhabitted,
// {
//     pub fn downgrade(self) -> locker::share_lock::RawShareGuard<'a, L> {
//         let g = std::mem::ManuallyDrop::new(self);
//         unsafe {
//             g.lock.downgrade();
//             crate::share_lock::RawShareGuard::from_raw(g.lock)
//         }
//     }
// }

impl<L: RawExclusiveLock + SplittableExclusiveLock + RawLockInfo> Clone
    for RawExclusiveGuard<'_, L>
{
    fn clone(&self) -> Self {
        Self::from_raw_parts((*self.inner).clone(), self.waker_set)
    }
}
