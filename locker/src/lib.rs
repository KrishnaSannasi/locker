#![cfg_attr(not(any(feature = "std", feature = "parking_lot_core")), no_std)]

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

pub unsafe trait RawLockInfo {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    type UniqueGuardTraits: Marker;
    type ShareGuardTraits: Marker;
}

pub(crate) use private::Sealed;
mod private {
    pub trait Sealed {}
}

pub trait Marker: Copy + Sealed {}
pub trait Inhabitted: Marker {}

impl Sealed for () {}
impl Marker for () {}
impl Inhabitted for () {}
impl Sealed for std::convert::Infallible {}
impl Marker for std::convert::Infallible {}

#[derive(Default, Clone, Copy)]
pub struct NoSend(std::marker::PhantomData<*const ()>);
unsafe impl Sync for NoSend {}

#[derive(Default, Clone, Copy)]
pub struct NoSync(std::marker::PhantomData<*const ()>);
unsafe impl Send for NoSync {}

impl Sealed for NoSend {}
impl Marker for NoSend {}
impl Inhabitted for NoSend {}

impl Sealed for NoSync {}
impl Marker for NoSync {}
impl Inhabitted for NoSync {}

impl<A: Sealed, B: Sealed> Sealed for (A, B) {}
impl<A: Marker, B: Marker> Marker for (A, B) {}
impl<A: Inhabitted, B: Inhabitted> Inhabitted for (A, B) {}

mod defer;
pub mod mutex;
pub mod once;
pub mod reentrant;
pub mod rwlock;
pub mod share_lock;
mod spin_wait;
pub mod unique_lock;

#[cfg(feature = "parking_lot_core")]
pub mod condvar;
#[cfg(feature = "parking_lot_core")]
pub mod waiter;
