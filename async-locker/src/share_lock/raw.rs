use crate::WakerSet;
use locker::share_lock::{RawShareGuard as Inner, RawShareLock, RawShareLockFair};
use locker::RawLockInfo;
use std::mem::ManuallyDrop;

pub struct RawShareGuard<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized> {
    inner: ManuallyDrop<Inner<'a, L>>,
    waker_set: &'a W,
}

impl<L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized> Drop for RawShareGuard<'_, L, W> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.inner);
            self.waker_set.notify_any();
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized> RawShareGuard<'a, L, W> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            /// # Safety
            ///
            /// The share lock must be held
            pub const fn from_raw_parts(inner: Inner<'a, L>, waker_set: &'a W) -> Self {
                Self { inner: ManuallyDrop::new(inner), waker_set }
            }
        } else {
            /// # Safety
            ///
            /// The share lock must be held
            pub fn from_raw_parts(inner: Inner<'a, L>, waker_set: &'a W) -> Self {
                Self { inner: ManuallyDrop::new(inner), waker_set }
            }
        }
    }
}

impl<'a, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized> RawShareGuard<'a, L, W> {
    pub fn inner(&self) -> &Inner<'a, L> {
        &self.inner
    }

    pub fn into_raw_parts(self) -> (Inner<'a, L>, &'a W) {
        let mut this = std::mem::ManuallyDrop::new(self);

        (
            unsafe { std::mem::ManuallyDrop::take(&mut this.inner) },
            this.waker_set,
        )
    }

    pub async fn bump(&self) {
        pub struct LockFuture<'a, 'b, L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized>(
            &'a RawShareGuard<'b, L, W>,
            Option<W::Index>,
        );

        pub struct LockOnDrop<'a>(&'a dyn RawShareLock);

        impl Drop for LockOnDrop<'_> {
            fn drop(&mut self) {
                self.0.shr_lock();
            }
        }

        use std::pin::Pin;
        use std::task::{Context, Poll};

        impl<L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized> std::future::Future
            for LockFuture<'_, '_, L, W>
        {
            type Output = ();

            fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
                let Self(mutex, opt_key) = Pin::into_inner(self);

                if let Some(key) = opt_key.take() {
                    mutex.waker_set.remove(key);
                }

                let inner = mutex.inner().inner();

                let key = if inner.shr_try_lock() {
                    return Poll::Ready(());
                } else {
                    mutex.waker_set.insert(ctx)
                };

                if inner.shr_try_lock() {
                    mutex.waker_set.remove(key);
                    Poll::Ready(())
                } else {
                    *opt_key = Some(key);
                    Poll::Pending
                }
            }
        }

        if !self.waker_set.is_empty() {
            {
                let raw = self.inner.inner();
                unsafe {
                    raw.shr_unlock();
                }

                // if waker_set.notify_any() panics we must not leave this guard in an unlocked state
                // as that would violate `RawShareGuard`'s safety invariants
                // so we bite the bullet and do a synchronous blocking lock
                // but given that if waker_set.notify_any() does panic, something has already gone horribly wrong
                // // this shouldn't be an issue. We also use dynamic dispatch in order to minimize codegen
                let _lock_on_drop = LockOnDrop(raw as _);

                self.waker_set.notify_any();

                std::mem::forget(_lock_on_drop);
            }

            LockFuture(self, None).await
        }
    }
}

impl<L: RawShareLock + RawLockInfo, W: WakerSet + ?Sized> Clone for RawShareGuard<'_, L, W> {
    fn clone(&self) -> Self {
        Self::from_raw_parts((*self.inner).clone(), self.waker_set)
    }
}
