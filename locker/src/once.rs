use crate::exclusive_lock::RawExclusiveLock;
use crate::RawLockInfo;

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

use std::ops::{Deref, DerefMut};

#[cfg(feature = "parking_lot_core")]
pub mod atomic;
#[cfg(feature = "parking_lot_core")]
pub mod local;

pub trait AsRawExclusiveLock {
    fn as_raw_exclusive_lock(&self) -> &dyn RawExclusiveLock;
}

impl<L: RawExclusiveLock> AsRawExclusiveLock for L {
    fn as_raw_exclusive_lock(&self) -> &dyn RawExclusiveLock {
        self
    }
}

pub unsafe trait Finish: RawExclusiveLock + AsRawExclusiveLock {
    fn is_done(&self) -> bool;

    fn mark_done(&self);

    fn get_and_mark_poisoned(&self) -> bool;
}

pub struct Once<L> {
    lock: L,
}

impl<L: RawLockInfo> Default for Once<L> {
    #[inline]
    fn default() -> Self {
        unsafe { Self::from_raw(L::INIT) }
    }
}

impl<L> Once<L> {
    /// # Safety
    ///
    /// * `lock` must not be shared, and must be freshly created
    #[inline]
    pub const unsafe fn from_raw(lock: L) -> Self {
        Self { lock }
    }
}

#[cfg(feature = "nightly")]
impl<L: Finish + RawLockInfo> Once<L> {
    #[inline]
    pub const fn new() -> Self {
        unsafe { Self::from_raw(L::INIT) }
    }
}

pub struct OnceState(bool);

impl OnceState {
    #[inline]
    pub const fn is_poisoned(&self) -> bool {
        self.0
    }
}

#[cold]
#[inline(never)]
fn force_call_once_slow(lock: &dyn Finish, use_lock: bool, f: &mut dyn FnMut(&OnceState)) {
    struct LocalGuard<'a>(&'a dyn RawExclusiveLock);

    impl Drop for LocalGuard<'_> {
        fn drop(&mut self) {
            unsafe { self.0.uniq_unlock() }
        }
    }

    let guard = if use_lock {
        lock.uniq_lock();
        Some(LocalGuard(lock.as_raw_exclusive_lock()))
    } else {
        None
    };

    let is_poisoned = lock.get_and_mark_poisoned();

    f(&OnceState(is_poisoned));

    lock.mark_done();

    drop(guard);
}

impl<L: Finish> Once<L> {
    #[inline]
    pub fn call_once(&self, f: impl FnOnce()) {
        if !self.lock.is_done() {
            self.call_once_inner(true, f);
        }
    }

    #[inline]
    pub fn call_once_mut(&mut self, f: impl FnOnce()) {
        if !self.lock.is_done() {
            self.call_once_inner(false, f);
        }
    }

    #[inline]
    pub fn force_call_once(&self, f: impl FnOnce(&OnceState)) {
        if !self.lock.is_done() {
            self.force_call_once_inner(true, f);
        }
    }

    #[inline]
    pub fn force_call_once_mut(&mut self, f: impl FnOnce(&OnceState)) {
        if !self.lock.is_done() {
            self.force_call_once_inner(false, f);
        }
    }

    #[inline]
    fn call_once_inner(&self, use_lock: bool, f: impl FnOnce()) {
        self.force_call_once_inner(use_lock, move |once_state: &OnceState| {
            assert!(
                !once_state.is_poisoned(),
                "tried to call `call_once*` on a poisoned `Once`"
            );

            f()
        });
    }

    #[inline]
    fn force_call_once_inner(&self, use_lock: bool, f: impl FnOnce(&OnceState)) {
        let mut f = Some(f);

        let mut f = move |once_state: &OnceState| f.take().unwrap()(once_state);

        force_call_once_slow(&self.lock, use_lock, &mut f);
    }
}

pub struct OnceCell<L, T> {
    once: Once<L>,
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<L, T: Send + Sync> Sync for OnceCell<L, T> where Once<L>: Sync {}

impl<L: RawLockInfo, T> Default for OnceCell<L, T> {
    #[inline]
    fn default() -> Self {
        unsafe { Self::from_once(Once::default()) }
    }
}

impl<L, T> OnceCell<L, T> {
    /// # Safety
    ///
    /// * `once` must be a freshly created `Once`
    #[inline]
    pub const unsafe fn from_once(once: Once<L>) -> Self {
        Self {
            once,
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

#[cfg(feature = "nightly")]
impl<L: Finish + RawLockInfo, T> OnceCell<L, T> {
    #[inline]
    pub const fn new() -> Self {
        unsafe { Self::from_once(Once::new()) }
    }
}

impl<L: Finish, T> OnceCell<L, T> {
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.once.lock.is_done() {
            unsafe { Some(&*self.value.get().cast::<T>()) }
        } else {
            None
        }
    }

    #[inline]
    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        let value = self.value.get().cast::<T>();

        self.once
            .force_call_once(|_once_state| unsafe { value.write(f()) });

        unsafe { &*value }
    }

    #[inline]
    pub fn get_or_init_mut(&mut self, f: impl FnOnce() -> T) -> &T {
        let value = self.value.get().cast::<T>();

        self.once
            .force_call_once_mut(|_once_state| unsafe { value.write(f()) });

        unsafe { &*value }
    }
}

enum LazyInner<F, T> {
    Func(F),
    Value(T),
    Empty,
}

pub enum Panic {}
pub enum Retry {}

pub struct Lazy<L, T, F, S> {
    once: Once<L>,
    inner: UnsafeCell<LazyInner<F, T>>,
    strategy: PhantomData<S>,
}

unsafe impl<L, F: Send + Sync, T: Send + Sync, S> Sync for Lazy<L, T, F, S> where Once<L>: Sync {}

impl<L: Finish + RawLockInfo, T, F: FnOnce() -> T> Lazy<L, T, F, Panic> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new(func: F) -> Self {
                unsafe { Self::from_raw_parts(Once::new(), func) }
            }
        } else {
            #[inline]
            pub fn new(func: F) -> Self {
                unsafe { Self::from_raw_parts(Once::default(), func) }
            }
        }
    }
}

impl<L: Finish + RawLockInfo, T, F: FnMut() -> T> Lazy<L, T, F, Retry> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "nightly")] {
            #[inline]
            pub const fn new_retry(func: F) -> Self {
                unsafe { Self::from_raw_parts(Once::new(), func) }
            }
        } else {
            #[inline]
            pub fn new_retry(func: F) -> Self {
                unsafe { Self::from_raw_parts(Once::default(), func) }
            }
        }
    }
}

impl<L, F, T, S> Lazy<L, T, F, S> {
    /// # Safety
    ///
    /// * `once` must be a freshly created `Once`
    #[inline]
    pub const unsafe fn from_raw_parts(once: Once<L>, func: F) -> Self {
        Self {
            once,
            strategy: PhantomData,
            inner: UnsafeCell::new(LazyInner::Func(func)),
        }
    }

    /// # Safety
    ///
    /// `Lazy::force` or `Lazy::force_mut` mut have been called before this
    #[inline]
    #[allow(unreachable_code)]
    pub unsafe fn get_unchecked(this: &Self) -> &T {
        if let LazyInner::Value(ref value) = *this.inner.get() {
            value
        } else {
            #[cfg(debug_assertions)]
            unreachable!("soundness hole");
            std::hint::unreachable_unchecked()
        }
    }

    /// # Safety
    ///
    /// `Lazy::force` or `Lazy::force_mut` mut have been called before this
    #[inline]
    #[allow(unreachable_code)]
    pub unsafe fn get_unchecked_mut(this: &mut Self) -> &mut T {
        if let LazyInner::Value(ref mut value) = *this.inner.get() {
            value
        } else {
            #[cfg(debug_assertions)]
            unreachable!("soundness hole");
            std::hint::unreachable_unchecked()
        }
    }
}

impl<L: Finish, F: FnOnce() -> T, T> Lazy<L, T, F, Panic> {
    #[inline]
    pub fn force(this: &Self) -> &T {
        let inner = this.inner.get();

        this.once.call_once(move || {
            let inner = unsafe { &mut *inner };
            let func = std::mem::replace(inner, LazyInner::Empty);

            if let LazyInner::Func(func) = func {
                *inner = LazyInner::Value(func());
            }
        });

        unsafe { Self::get_unchecked(this) }
    }

    #[inline]
    pub fn force_mut(this: &mut Self) -> &mut T {
        let inner = this.inner.get();

        this.once.call_once(move || {
            let inner = unsafe { &mut *inner };
            let func = std::mem::replace(inner, LazyInner::Empty);

            if let LazyInner::Func(func) = func {
                *inner = LazyInner::Value(func());
            }
        });

        unsafe { Self::get_unchecked_mut(this) }
    }
}

impl<L: Finish, F: FnMut(&OnceState) -> T, T> Lazy<L, T, F, Retry> {
    #[inline]
    pub fn force(this: &Self) -> &T {
        let inner = this.inner.get();

        this.once.force_call_once(move |once_state| {
            let inner = unsafe { &mut *inner };

            if let LazyInner::Func(ref mut func) = *inner {
                *inner = LazyInner::Value(func(once_state));
            }
        });

        unsafe { Self::get_unchecked(this) }
    }

    #[inline]
    pub fn force_mut(this: &mut Self) -> &mut T {
        let inner = this.inner.get();

        this.once.force_call_once(move |once_state| {
            let inner = unsafe { &mut *inner };

            if let LazyInner::Func(ref mut func) = *inner {
                *inner = LazyInner::Value(func(once_state));
            }
        });

        unsafe { Self::get_unchecked_mut(this) }
    }
}

impl<L: Finish, F: FnOnce() -> T, T> Deref for Lazy<L, T, F, Panic> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}

impl<L: Finish, F: FnOnce() -> T, T> DerefMut for Lazy<L, T, F, Panic> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        Self::force_mut(self)
    }
}

impl<L: Finish, F: FnMut(&OnceState) -> T, T> Deref for Lazy<L, T, F, Retry> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        Self::force(self)
    }
}

impl<L: Finish, F: FnMut(&OnceState) -> T, T> DerefMut for Lazy<L, T, F, Retry> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        Self::force_mut(self)
    }
}

// #[test]
// #[should_panic(expected = "tried to call `call_once*` on a poisoned `Once`")]
// pub fn second_force_should_panic() {
//     use std::sync::atomic::{AtomicBool, Ordering};
//     static ATOMIC: AtomicBool = AtomicBool::new(true);
//     static ONCE: prelude::Lazy<u32> = prelude::Lazy::new(|| {
//         if ATOMIC.swap(false, Ordering::Relaxed) {
//             panic!();
//         }

//         0xDEAD_BEEF
//     });

//     let _ = std::panic::catch_unwind(move || prelude::Lazy::force(&ONCE));

//     // this should panic due to a poisoned `Once`
//     prelude::Lazy::force(&ONCE);
// }

// #[test]
// pub fn second_force_should_retry() {
//     type Lazy = prelude::RetryLazy<u32>;

//     use std::sync::atomic::{AtomicBool, Ordering};
//     static ATOMIC: AtomicBool = AtomicBool::new(true);
//     static ONCE: Lazy = Lazy::new(|_once_state| {
//         if ATOMIC.swap(false, Ordering::Relaxed) {
//             panic!();
//         }

//         0xDEAD_BEEF
//     });

//     let _ = std::panic::catch_unwind(move || Lazy::force(&ONCE));
//     assert_eq!(*Lazy::force(&ONCE), 0xDEAD_BEEF);
// }
