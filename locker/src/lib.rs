#![deny(missing_docs)]
#![cfg_attr(not(any(test, feature = "std", feature = "parking_lot_core")), no_std)]
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

#[cfg(not(any(test, feature = "std", feature = "parking_lot_core")))]
extern crate core as std;

#[cfg(all(feature = "alloc", not(test), not(feature = "std")))]
extern crate alloc;

mod alloc_prelude {
    cfg_if::cfg_if! {
        if #[cfg(any(test, feature="std"))] {
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

/// Create an item at compile time
pub trait Init: Sized {
    #[allow(clippy::declare_interior_mutable_const)]
    /// A default initial value for the lock
    const INIT: Self;
}

/// Some basic information about raw locks, like how to create them and
/// what traits their guards should implement
///
/// # Safety
///
/// * there can be no way to safely change the lock state
/// outside of the trait methods provided by this crate
/// * `ExclusiveGuardTraits` & `ShareGuardTraits`: These fields contain types that will
/// go directly into each of the `Raw*Guard` types. They can control what auto-traits are
/// implemented, use these to limit the `Send` and `Sync` bounds on the guards.
/// You can use `NoSend` to remove the `Send` bounds, and `NoSync` to remove the `Sync` bound.
/// To remove both, you can use `(NoSend, NoSync)`
/// If it is should be impossible to create the guard, then use `std::convert::Infallible`
pub unsafe trait RawLockInfo {
    /// A type that will remove auto-trait implementations for the `*ExclusiveGuard` types
    type ExclusiveGuardTraits: marker::Marker;

    /// A type that will remove auto-trait implementations for the `*ShareGuard` types
    type ShareGuardTraits: marker::Marker;
}

/// Used in the `*LockTimed` traits
///
/// This is extracted out because if both `RawExclusiveLockTimed` and `RawShareLockTimed`
/// are implemented, then they should both use the same instant and duration
///
/// The `Duration` and `Instant` types are specified as associated types so that
/// this trait is usable even in no_std environments.
pub trait RawTimedLock: RawLockInfo {
    /// Instant type used for `try_lock_until`.
    type Instant;

    /// Duration type used for `try_lock_until`.
    type Duration;
}

pub mod combinators;
mod defer;
pub mod exclusive_lock;
pub mod mutex;
#[allow(missing_docs)]
pub mod once; // 68
              // #[allow(missing_docs)]
pub mod remutex;
pub mod rwlock;
pub mod share_lock;
mod spin_wait;

#[allow(missing_docs)]
#[cfg(feature = "parking_lot_core")]
pub mod condvar; // 25
mod guard;
pub mod marker;
#[allow(missing_docs)]
#[cfg(feature = "parking_lot_core")]
pub mod waiter; // 25

pub use guard::{Mapped, Pure, TryMapError};
use marker::*;
