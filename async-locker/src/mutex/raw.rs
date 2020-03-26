use crate::{exclusive_lock::raw::RawExclusiveGuard, waker_set::WakerSet};

use locker::mutex::{raw, RawMutex};

pub struct Mutex<L> {
    raw: raw::Mutex<L>,
    waker_set: WakerSet,
}

impl<L: RawMutex + locker::Init> Default for Mutex<L> {
    #[inline]
    fn default() -> Self {
        locker::Init::INIT
    }
}

impl<L> Mutex<L> {
    /// # Safety
    ///
    /// You must pass `RawLockInfo::INIT` as lock
    #[inline]
    pub const fn from_raw(raw: raw::Mutex<L>) -> Self {
        Self {
            raw,
            waker_set: WakerSet::new(),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (raw::Mutex<L>, WakerSet) {
        (self.raw, self.waker_set)
    }

    #[inline]
    pub const fn raw_mutex(&self) -> &raw::Mutex<L> {
        &self.raw
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const unsafe fn raw_mutex_mut(&mut self) -> &mut raw::Mutex<L> {
                &mut self.lock
            }
        } else {
            #[inline]
            pub unsafe fn raw_mutex_mut(&mut self) -> &mut raw::Mutex<L> {
                &mut self.raw
            }
        }
    }
}

impl<L: RawMutex + locker::Init> locker::Init for Mutex<L> {
    const INIT: Self = unsafe { Self::from_raw(locker::Init::INIT) };
}

impl<L: RawMutex + locker::Init> Mutex<L> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new() -> Self {
                locker::Init::INIT
            }
        } else {
            #[inline]
            pub fn new() -> Self {
                locker::Init::INIT
            }
        }
    }
}

impl<L: RawMutex> Mutex<L>
where
    L::ExclusiveGuardTraits: locker::marker::Inhabitted,
{
    #[inline]
    pub async fn lock(&self) -> RawExclusiveGuard<'_, L> {
        use crate::slab::Index;

        pub struct LockFuture<'a, L>(&'a Mutex<L>, Option<Index>);

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<'a, L: RawMutex> std::future::Future for LockFuture<'a, L>
        where
            L::ExclusiveGuardTraits: locker::marker::Inhabitted,
        {
            type Output = RawExclusiveGuard<'a, L>;

            fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
                let Self(mutex, opt_key) = Pin::into_inner(self);

                if let Some(key) = opt_key.take() {
                    mutex.waker_set.remove(key);
                }

                let key = match mutex.try_lock() {
                    Some(gaurd) => return Poll::Ready(gaurd),
                    None => mutex.waker_set.insert(ctx),
                };

                match mutex.try_lock() {
                    Some(gaurd) => {
                        mutex.waker_set.remove(key);
                        Poll::Ready(gaurd)
                    }
                    None => {
                        *opt_key = Some(key);
                        Poll::Pending
                    }
                }
            }
        }

        LockFuture(self, None).await
    }

    #[inline]
    pub fn try_lock(&self) -> Option<RawExclusiveGuard<'_, L>> {
        let guard = self.raw.try_lock()?;

        Some(RawExclusiveGuard::from_raw_parts(guard, &self.waker_set))
    }
}
