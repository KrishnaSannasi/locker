#![cfg_attr(not(any(feature = "std", feature = "parking_lot_core")), no_std)]
#![cfg_attr(
    feature = "nightly",
    feature(
        optin_builtin_traits,
        const_fn,
        const_mut_refs,
        const_raw_ptr_deref,
    ),
)]

#[cfg(not(any(feature = "std", feature = "parking_lot_core")))]
extern crate core as std;

#[cfg(all(feature = "alloc", not(feature = "std")))]
extern crate alloc;

mod alloc_prelude {
    cfg_if::cfg_if! {
        if #[cfg(feature="std")] {
            pub use std::boxed::Box;
            pub use std::sync::Arc;
            pub use std::rc::Rc;
        } else if #[cfg(feature="alloc")] {
            pub use alloc::boxed::Box;
            pub use alloc::sync::Arc;
            pub use alloc::rc::Rc;
        }
    }
}

macro_rules! defer {
    ($($inner:tt)*) => {
        let _defer = crate::defer::Defer::new(|| $($inner)*);
    };
}

/// # Safety
/// 
/// * there can be no way to safely change the lock state
/// outside of the trait methods provided by this crate
/// * `INIT` (TODO)
/// * `ExclusiveGuardTraits` (TODO)
/// * `ShareGuardTraits` (TODO)
pub unsafe trait RawLockInfo {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    type ExclusiveGuardTraits: Marker;
    type ShareGuardTraits: Marker;
}

pub trait Marker: Copy {}
pub trait Inhabitted: Marker {
    const INIT: Self;
}

impl Marker for () {}
impl Inhabitted for () {
    const INIT: Self = ();
}
impl Marker for std::convert::Infallible {}

cfg_if::cfg_if! {
    if #[cfg(feature = "nightly")] {
        #[derive(Default, Clone, Copy)]
        pub struct NoSend(std::marker::PhantomData<()>);
        impl !Send for NoSync {}

        #[derive(Default, Clone, Copy)]
        pub struct NoSync(std::marker::PhantomData<()>);
        impl !Sync for NoSync {}
    } else {
        #[derive(Default, Clone, Copy)]
        pub struct NoSend(std::marker::PhantomData<*const ()>);
        unsafe impl Sync for NoSend {}

        #[derive(Default, Clone, Copy)]
        pub struct NoSync(std::marker::PhantomData<*const ()>);
        unsafe impl Send for NoSync {}
    }
}

impl Marker for NoSend {}
impl Inhabitted for NoSend {
    const INIT: Self = Self(std::marker::PhantomData);
}

impl Marker for NoSync {}
impl Inhabitted for NoSync {
    const INIT: Self = Self(std::marker::PhantomData);
}

impl<A: Marker, B: Marker> Marker for (A, B) {}
impl<A: Inhabitted, B: Inhabitted> Inhabitted for (A, B) {
    const INIT: Self = (A::INIT, B::INIT);
}

pub mod combinators;
mod defer;
pub mod exclusive_lock;
pub mod mutex;
pub mod once;
pub mod reentrant;
pub mod rwlock;
pub mod share_lock;
mod spin_wait;

#[cfg(feature = "parking_lot_core")]
pub mod condvar;
#[cfg(feature = "parking_lot_core")]
pub mod waiter;
