use std::cell::UnsafeCell;
use std::num::NonZeroUsize;

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::share_lock::{RawShareLock, RawShareLockFair, RawShareLockExt, ShareGuard};
use crate::exclusive_lock::{RawExclusiveLock, RawExclusiveLockFair};

#[cfg(feature = "std")]
pub mod prelude {
    pub type ReentrantMutex<L, T> = super::ReentrantMutex<L, super::StdThreadInfo, T>;
    pub type ReentrantLock<L> = super::ReentrantLock<L, super::StdThreadInfo>;
}

/// # Safety
///
/// Implementations of this trait must ensure that no two active threads share
/// the same thread ID. However the ID of a thread that has exited can be re-used
/// since that thread is no longer active.
pub unsafe trait ThreadInfo {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    fn id(&self) -> NonZeroUsize;
}

pub struct ReentrantMutex<L, I, T: ?Sized> {
    lock: ReentrantLock<L, I>,
    value: UnsafeCell<T>,
}

pub struct ReentrantLock<L, I> {
    inner: L,
    thread_info: I,
    owner: AtomicUsize,
    count: Cell<usize>,
}

impl<L, I> ReentrantLock<L, I> {
    /// # Safety
    ///
    /// `inner` must not be shared
    #[inline]
    pub const unsafe fn from_raw_parts(inner: L, thread_info: I) -> Self {
        Self {
            inner,
            thread_info,
            owner: AtomicUsize::new(0),
            count: Cell::new(0),
        }
    }
}

#[cfg(feature = "std")]
pub struct StdThreadInfo;

#[cfg(feature = "std")]
unsafe impl ThreadInfo for StdThreadInfo {
    const INIT: Self = Self;

    #[inline]
    fn id(&self) -> NonZeroUsize {
        use std::mem::MaybeUninit;

        thread_local! {
            static IDS: MaybeUninit<u8> = MaybeUninit::uninit();
        }

        IDS.with(|x| unsafe { NonZeroUsize::new_unchecked(x as *const MaybeUninit<u8> as usize) })
    }
}

unsafe impl<L: crate::RawLockInfo, I: ThreadInfo> crate::RawLockInfo for ReentrantLock<L, I> {
    const INIT: Self = unsafe { Self::from_raw_parts(L::INIT, I::INIT) };

    type ExclusiveGuardTraits = std::convert::Infallible;
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

impl<L: RawExclusiveLock, I: ThreadInfo> ReentrantLock<L, I> {
    #[inline]
    fn lock_internal(&self, try_lock: impl FnOnce() -> bool) -> bool {
        let id = self.thread_info.id().get();
        let owner = self.owner.load(Ordering::Relaxed);

        if owner == id {
            unsafe { self.shr_split() }
        } else {
            if !try_lock() {
                return false
            }

            self.owner.store(id, Ordering::Relaxed);
            debug_assert_eq!(self.count.get(), 0);
        }

        true
    }

    #[inline]
    fn unlock_internal(&self, unlock_slow: impl FnOnce()) {
        if let Some(count) = self.count.get().checked_sub(1) {
            self.count.set(count);
        } else {
            unlock_slow()
        }
    }
}

unsafe impl<L: RawExclusiveLock, I: ThreadInfo> RawShareLock for ReentrantLock<L, I> {
    #[inline]
    fn shr_lock(&self) {
        self.lock_internal(|| {
            self.inner.uniq_lock();
            true
        });
    }

    #[inline]
    fn shr_try_lock(&self) -> bool {
        self.lock_internal(|| self.inner.uniq_try_lock())
    }

    #[inline]
    unsafe fn shr_split(&self) {
        #[cfg(debug_assertions)]
        {
            assert_eq!(
                self.owner.load(Ordering::Relaxed),
                self.thread_info.id().get()
            );
        }

        self.count.set(
            self.count
                .get()
                .checked_add(1)
                .expect("tried to create too many reentrant locks"),
        );
    }

    #[inline]
    unsafe fn shr_unlock(&self) {
        self.unlock_internal(#[cold] || self.inner.uniq_unlock())
    }

    #[inline]
    unsafe fn shr_bump(&self) {
        if self.count.get() == 0 {
            self.inner.uniq_bump();
        }
    }
}

unsafe impl<L: RawExclusiveLockFair, I: ThreadInfo> RawShareLockFair for ReentrantLock<L, I> {
    #[inline]
    unsafe fn shr_unlock_fair(&self) {
        self.unlock_internal(#[cold] || self.inner.uniq_unlock_fair())
    }

    #[inline]
    unsafe fn shr_bump_fair(&self) {
        if self.count.get() == 0 {
            self.inner.uniq_bump_fair();
        }
    }
}

impl<L: crate::RawLockInfo, I: ThreadInfo, T: Default> Default for ReentrantMutex<L, I, T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<L: Sync, I: Sync, T: Send> Sync for ReentrantMutex<L, I, T> {}
unsafe impl<L: Sync, I: Sync> Sync for ReentrantLock<L, I> {}

impl<L, I, T> ReentrantMutex<L, I, T> {
    /// # Safety
    ///
    /// You must pass `RawUniueLock::INIT` as lock
    #[inline]
    pub const unsafe fn from_raw_parts(lock: ReentrantLock<L, I>, value: T) -> Self {
        Self {
            lock,
            value: UnsafeCell::new(value),
        }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (ReentrantLock<L, I>, T) {
        (self.lock, self.value.into_inner())
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, I, T: ?Sized> ReentrantMutex<L, I, T> {
    #[inline]
    pub unsafe fn raw(&self) -> &ReentrantLock<L, I> {
        &self.lock
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

impl<L: crate::RawLockInfo, I: ThreadInfo, T> ReentrantMutex<L, I, T> {
    #[inline]
    pub fn new(value: T) -> Self {
        unsafe { Self::from_raw_parts(ReentrantLock::from_raw_parts(L::INIT, I::INIT), value) }
    }
}

impl<L: RawExclusiveLock + crate::RawLockInfo, I: ThreadInfo, T: ?Sized> ReentrantMutex<L, I, T> {
    #[inline]
    pub fn lock(&self) -> ShareGuard<'_, ReentrantLock<L, I>, T> {
        ShareGuard::new(self.lock.raw_shr_lock(), unsafe { &mut *self.value.get() })
    }

    #[inline]
    pub fn try_lock(&self) -> Option<ShareGuard<'_, ReentrantLock<L, I>, T>> {
        Some(ShareGuard::new(self.lock.try_raw_shr_lock()?, unsafe {
            &mut *self.value.get()
        }))
    }
}

#[test]
fn reentrant() {
    use crate::mutex::simple::RawLock;

    type ReentrantMutex<T> = prelude::ReentrantMutex<RawLock, T>;

    let mtx = ReentrantMutex::new(Cell::new(0));

    let _lock = mtx.lock();

    assert_eq!(_lock.get(), 0);

    mtx.lock().set(10);

    assert_eq!(_lock.get(), 10);
}

#[test]
fn reentrant_multi() {
    use crate::mutex::simple::RawLock;

    type ReentrantMutex<T> = prelude::ReentrantMutex<RawLock, T>;

    let mtx = ReentrantMutex::new(Cell::new(0));
    let mtx = std::sync::Arc::new(mtx);

    let t = std::thread::spawn({
        let mtx = mtx.clone();
        move || {
            assert!(mtx.try_lock().is_none());
            let _lock = mtx.lock();

            assert!(_lock.get() == 0 || _lock.get() == 10);
        }
    });

    let _lock = mtx.lock();

    // provide enough time for the thread to initialize
    std::thread::sleep(std::time::Duration::from_micros(10));

    assert_eq!(_lock.get(), 0);

    mtx.lock().set(10);

    assert_eq!(_lock.get(), 10);

    drop(_lock);

    t.join().unwrap();
}
