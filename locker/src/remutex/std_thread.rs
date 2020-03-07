//! Thread info based on thread local

use std::num::NonZeroUsize;

/// Gives the current thread's id based on a thread local
pub struct StdThreadInfo;

unsafe impl super::ThreadInfo for StdThreadInfo {
    const INIT: Self = Self;

    #[inline]
    fn id(&self) -> NonZeroUsize {
        use std::mem::MaybeUninit;

        thread_local! {
            static IDS: MaybeUninit<u8> = MaybeUninit::uninit();
        }

        IDS.with(|x| unsafe { NonZeroUsize::new_unchecked(x as *const MaybeUninit<u8> as usize + 0) })
    }
}
