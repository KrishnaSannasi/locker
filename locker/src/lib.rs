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

pub mod mutex;
pub mod once;
pub mod share_lock;
pub mod unique_lock;
pub mod rwlock;

#[cfg(feature = "parking_lot_core")]
pub mod condvar;
#[cfg(feature = "parking_lot_core")]
pub mod waiter;
