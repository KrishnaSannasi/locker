//! A bunch of useful combinators that can by applied to any lock

mod always_fair;
pub use always_fair::Fair;

mod reentrant_panic;
pub use reentrant_panic::ReentrantPanic;
