#![allow(missing_docs)]

pub unsafe trait Scalar: Copy {
    /// A value representing zero
    ///
    /// `Self::ZERO.dec() == None`
    const ZERO: Self;

    fn to_usize(self) -> usize;

    fn is_in_bounds(_: usize) -> bool;

    fn from_usize_unchecked(_: usize) -> Self;
}

#[cfg(target_pointer_width = "16")]
const SUB_WORD_COUNT: usize = 1;

#[cfg(target_pointer_width = "32")]
const SUB_WORD_COUNT: usize = 3;

#[cfg(target_pointer_width = "64")]
const SUB_WORD_COUNT: usize = 7;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SubWord([u8; SUB_WORD_COUNT]);

unsafe impl Scalar for SubWord {
    const ZERO: Self = Self([0; SUB_WORD_COUNT]);

    fn to_usize(self) -> usize {
        let Self(bytes) = self;
        #[cfg(target_pointer_width = "16")]
        {
            usize::from_le_bytes([bytes[0], 0])
        }

        #[cfg(target_pointer_width = "32")]
        {
            usize::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])
        }

        #[cfg(target_pointer_width = "64")]
        {
            usize::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], 0,
            ])
        }
    }

    fn is_in_bounds(word: usize) -> bool {
        word < 1 << (SUB_WORD_COUNT * 8)
    }

    fn from_usize_unchecked(word: usize) -> Self {
        #[cfg(target_pointer_width = "16")]
        {
            let bytes = word.to_le_bytes();
            assert!(bytes[2] == 0, "Cannot overflow");
            Self([bytes[0]])
        }

        #[cfg(target_pointer_width = "32")]
        {
            let bytes = word.to_le_bytes();
            assert!(bytes[3] == 0, "Cannot overflow");
            Self([bytes[0], bytes[1], bytes[2]])
        }

        #[cfg(target_pointer_width = "64")]
        {
            let bytes = word.to_le_bytes();
            assert!(bytes[7] == 0, "Cannot overflow");
            Self([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
            ])
        }
    }
}
