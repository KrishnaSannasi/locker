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

const WORD_SIZE: usize = std::mem::size_of::<usize>();
const SUB_WORD_SIZE: usize = WORD_SIZE - 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SubWord([u8; SUB_WORD_SIZE]);

unsafe impl Scalar for SubWord {
    const ZERO: Self = Self([0; SUB_WORD_SIZE]);

    fn to_usize(self) -> usize {
        let Self(bytes) = self;

        bytes.to_usize()
    }

    fn is_in_bounds(word: usize) -> bool {
        <[u8; SUB_WORD_SIZE] as Scalar>::is_in_bounds(word)
    }

    fn from_usize_unchecked(word: usize) -> Self {
        Self(<[u8; SUB_WORD_SIZE] as Scalar>::from_usize_unchecked(word))
    }
}

macro_rules! integers {
    ($($int:ident)*) => {$(
        unsafe impl Scalar for $int {
            const ZERO: Self = 0;

            fn to_usize(self) -> usize {
                use std::convert::TryFrom;

                usize::try_from(self).expect("too large to convert")
            }

            fn is_in_bounds(word: usize) -> bool {
                use std::convert::TryFrom;

                Self::try_from(word).is_ok()
            }

            fn from_usize_unchecked(word: usize) -> Self {
                word as Self
            }
        }
    )*}
}

integers! { u8 u16 u32 u64 }

macro_rules! byte_arrays {
    ($($count:literal)*) => {$(
        unsafe impl Scalar for [u8; $count] {
            const ZERO: Self = [0; $count];

            fn to_usize(self) -> usize {
                let mut array = [0; WORD_SIZE];

                unsafe {
                    (&mut array as *mut [u8; WORD_SIZE] as *mut Self).write(self);
                }

                usize::from_le_bytes(array)
            }

            fn is_in_bounds(word: usize) -> bool {
                1_usize.checked_shl($count * 8).map_or(true, |max| word < max)
            }

            fn from_usize_unchecked(word: usize) -> Self {
                let array = word.to_le_bytes();

                unsafe {
                    (&array as *const [u8; WORD_SIZE] as *const Self).read()
                }
            }
        }
    )*};
}

byte_arrays! { 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 }
