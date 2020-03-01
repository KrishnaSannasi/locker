#![deny(missing_docs)]
#![cfg_attr(not(any(feature = "std", feature = "parking_lot_core")), no_std)]
#![cfg_attr(
    feature = "nightly",
    feature(
        optin_builtin_traits,
        const_fn,
        const_mut_refs,
        const_raw_ptr_deref,
        const_loop,
    )
)]

//! # locker
//!
//! A reimplementation of lock-api and parking_lot where the abstractions are
//! integrated together more seemlessly and without too much code duplication.
//!

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

/// Some basic information about raw locks, like how to create them and
/// what traits their guards should implement
///
/// # Safety
///
/// * there can be no way to safely change the lock state
/// outside of the trait methods provided by this crate
/// * `INIT`: It must be safe to use `INIT` as the initail value for the lock
/// * `ExclusiveGuardTraits` & `ShareGuardTraits`: These fields contain types that will
/// go directly into each of the `Raw*Guard` types. They can control what auto-traits are
/// implemented, use these to limit the `Send` and `Sync` bounds on the guards.
/// You can use `NoSend` to remove the `Send` bounds, and `NoSync` to remove the `Sync` bound.
/// To remove both, you can use `(NoSend, NoSync)`
/// If it is should be impossible to create the guard, then use `std::convert::Infallible`
pub unsafe trait RawLockInfo {
    #[allow(clippy::declare_interior_mutable_const)]
    /// A default initial value for the lock
    const INIT: Self;

    /// A type that will remove auto-trait implementations for the `*ExclusiveGuard` types
    type ExclusiveGuardTraits: Marker;

    /// A type that will remove auto-trait implementations for the `*ShareGuard` types
    type ShareGuardTraits: Marker;
}

/// A marker to be used with [`RawLockInfo::*GuardTraits`](crate::RawLockInfo)
/// to remove auto-trait implementations
///
/// These should be zero-sized
pub trait Marker: Copy {}

/// A [`Marker`](crate::Marker)
///
/// These should be zero-sized
pub trait Inhabitted: Marker {
    // # Safety note
    //
    // This must be a const to prevent running user controlled code
    // in the construction of the guards, the `INIT` serves as proof of
    // work that this is in fact an inhabitted type. (This prevents an implementaiton)
    // of the never type for `Inhabitted`

    /// A value of `Self`
    ///
    const INIT: Self;
}

impl Marker for () {}
impl Inhabitted for () {
    const INIT: Self = ();
}
impl Marker for std::convert::Infallible {}

/// A [`Marker`](crate::Marker) that doesn't implement `Send`
#[derive(Default, Clone, Copy)]
pub struct NoSend(Inner);

/// A [`Marker`](crate::Marker) that doesn't implement `Sync`
#[derive(Default, Clone, Copy)]
pub struct NoSync(Inner);

cfg_if::cfg_if! {
    if #[cfg(feature = "nightly")] {
        type Inner = std::marker::PhantomData<()>;
        impl !Send for NoSync {}
        impl !Sync for NoSync {}
    } else {
        type Inner = std::marker::PhantomData<&'static std::cell::Cell<()>>;
        unsafe impl Sync for NoSend {}
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
#[allow(missing_docs)]
mod defer;
#[allow(missing_docs)]
pub mod exclusive_lock;
#[allow(missing_docs)]
pub mod mutex;
#[allow(missing_docs)]
pub mod once;
#[allow(missing_docs)]
pub mod reentrant;
#[allow(missing_docs)]
pub mod rwlock;
#[allow(missing_docs)]
pub mod share_lock;
#[allow(missing_docs)]
mod spin_wait;

#[allow(missing_docs)]
#[cfg(feature = "parking_lot_core")]
pub mod condvar;
#[allow(missing_docs)]
#[cfg(feature = "parking_lot_core")]
pub mod waiter;
