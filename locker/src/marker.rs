//! Markers that specify what auto traits can't be implemented for guards

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
