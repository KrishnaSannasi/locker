use std::cell::UnsafeCell;
use std::num::NonZeroUsize;

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::share_lock::{RawShareLock, RawShareLockExt, ShareGuard};
use crate::unique_lock::RawUniqueLock;

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
    unique: L,
    thread_info: I,
    thread: AtomicUsize,
    count: Cell<usize>,
}

impl<L, I> ReentrantLock<L, I> {
    /// # Safety
    ///
    /// `unique` must not be shared
    pub const unsafe fn from_raw_parts(unique: L, thread_info: I) -> Self {
        Self {
            unique,
            thread_info,
            thread: AtomicUsize::new(0),
            count: Cell::new(0),
        }
    }
}

#[cfg(feature = "std")]
pub struct StdThreadInfo;

#[cfg(feature = "std")]
unsafe impl ThreadInfo for StdThreadInfo {
    const INIT: Self = Self;

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

    type UniqueGuardTraits = std::convert::Infallible;
    type ShareGuardTraits = (crate::NoSend, crate::NoSync);
}

unsafe impl<L: RawUniqueLock, I: ThreadInfo> RawShareLock for ReentrantLock<L, I> {
    fn shr_lock(&self) {
        let id = self.thread_info.id().get();

        if self.unique.uniq_try_lock() {
            // we acquired a unique lock
        } else {
            let old_id = self.thread.compare_and_swap(0, id, Ordering::Relaxed);

            if old_id == 0 || old_id == id {
                unsafe { self.shr_split() }
                return;
            } else {
                self.unique.uniq_lock();
            }
        }

        // we acquired a unique lock

        self.thread.store(id, Ordering::Relaxed);
        self.count.set(0);
    }

    fn shr_try_lock(&self) -> bool {
        let id = self.thread_info.id().get();

        if self.unique.uniq_try_lock() {
            // we acquired a unique lock

            self.thread.store(id, Ordering::Relaxed);
            self.count.set(0);

            true
        } else {
            let old_id = self.thread.compare_and_swap(0, id, Ordering::Relaxed);

            if old_id == 0 || old_id == id {
                unsafe { self.shr_split() }
                true
            } else {
                false
            }
        }
    }

    unsafe fn shr_split(&self) {
        #[cfg(debug_assertions)]
        {
            assert_eq!(
                self.thread.load(Ordering::Relaxed),
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

    unsafe fn shr_unlock(&self) {
        let count = self.count.get();
        if let Some(x) = count.checked_sub(1) {
            self.count.set(x)
        } else {
            // if all reentrant locks are released

            self.unique.uniq_unlock()
        }
    }
}

impl<L: crate::RawLockInfo, I: ThreadInfo, T: Default> Default for ReentrantMutex<L, I, T> {
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
    pub const unsafe fn from_raw_parts(lock: ReentrantLock<L, I>, value: T) -> Self {
        Self {
            lock,
            value: UnsafeCell::new(value),
        }
    }

    pub fn into_raw_parts(self) -> (ReentrantLock<L, I>, T) {
        (self.lock, self.value.into_inner())
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<L, I, T: ?Sized> ReentrantMutex<L, I, T> {
    pub unsafe fn raw(&self) -> &ReentrantLock<L, I> {
        &self.lock
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

impl<L: crate::RawLockInfo, I: ThreadInfo, T> ReentrantMutex<L, I, T> {
    pub fn new(value: T) -> Self {
        unsafe { Self::from_raw_parts(ReentrantLock::from_raw_parts(L::INIT, I::INIT), value) }
    }
}

impl<L: RawUniqueLock + crate::RawLockInfo, I: ThreadInfo, T: ?Sized> ReentrantMutex<L, I, T> {
    pub fn lock(&self) -> ShareGuard<'_, ReentrantLock<L, I>, T> {
        ShareGuard::new(self.lock.raw_shr_lock(), unsafe { &mut *self.value.get() })
    }

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
