pub mod guard;
#[doc(hidden)]
pub mod raw;

pub use guard::ExclusiveGuard;
pub use raw::RawExclusiveGuard;
