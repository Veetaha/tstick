mod webm_vp9_two_pass;
mod single_gen;
mod multi_gen;

#[cfg(test)]
mod testing;

use crate::util::byte_size::KIB;

pub(crate) use multi_gen::MultiVideoGenContext;

const MAX_EMOJI_BYTES: usize = 64 * KIB;
const MAX_STICKER_BYTES: usize = 256 * KIB;

const EMOJI_BOUNDING_BOX: u64 = 100;
const STICKER_BOUNDING_BOX: u64 = 512;

/// Max value of CRF according to [the docs](https://trac.ffmpeg.org/wiki/Encode/VP9)
const MAX_CRF: usize = 63;

#[derive(strum::Display, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum PackEntryKind {
    Emoji,
    Sticker,
}

impl PackEntryKind {
    fn max_bytes(&self) -> usize {
        match self {
            Self::Emoji => MAX_EMOJI_BYTES,
            Self::Sticker => MAX_STICKER_BYTES,
        }
    }

    /// Telegram supports rectangle stickers, but not emojis.
    fn must_be_square(&self) -> bool {
        match self {
            Self::Emoji => true,
            Self::Sticker => false,
        }
    }

    fn bounding_box(&self) -> u64 {
        match self {
            Self::Emoji => EMOJI_BOUNDING_BOX,
            Self::Sticker => STICKER_BOUNDING_BOX,
        }
    }
}
