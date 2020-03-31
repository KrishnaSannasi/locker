#![allow(unused, clippy::missing_safety_doc)]

use core::task::Context;

macro_rules! defer {
    ($($inner:tt)*) => {
        let _defer = crate::defer::Defer::new(|| $($inner)*);
    };
}

pub mod async_std;
mod defer;
pub mod exclusive_lock;
pub mod local_async_std;
pub mod mutex;
pub mod remutex;
pub mod rwlock;
pub mod share_lock;
mod slab;

pub trait WakerSet {
    type Index: std::marker::Unpin;

    fn insert(&self, cx: &mut Context) -> Self::Index;
    fn is_empty(&self) -> bool;
    fn remove(&self, key: Self::Index);
    fn cancel(&self, key: Self::Index) -> bool;
    fn notify_any(&self) -> bool;
    fn notify_all(&self) -> bool;
}
