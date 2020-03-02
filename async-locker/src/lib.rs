macro_rules! defer {
    ($($inner:tt)*) => {
        let _defer = crate::defer::Defer::new(|| $($inner)*);
    };
}

mod defer;
pub mod exclusive_lock;
pub mod mutex;
pub mod share_lock;
mod slab;
mod waker_set;
