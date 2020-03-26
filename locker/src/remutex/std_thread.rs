//! Thread info based on thread local

use core::num::NonZeroUsize;

/// Gives the current thread's id based on a thread local
pub struct StdThreadInfo;

impl crate::Init for StdThreadInfo {
    const INIT: Self = Self;
}

unsafe impl super::ThreadInfo for StdThreadInfo {
    #[inline]
    fn id(&self) -> NonZeroUsize {
        use core::mem::MaybeUninit;

        thread_local! {
            static IDS: MaybeUninit<u8> = MaybeUninit::uninit();
        }

        IDS.with(|x| unsafe { NonZeroUsize::new_unchecked(x as *const MaybeUninit<u8> as usize) })
    }
}
