//! This is a module defining well-known codec families and their parameters.
//!
//! Some transports will not need to use this module, especially if they have their own bespoke transport mechanism.
//! However, transports that use common encoding/decoding mechanisms may benefit from using this module.
//!
//! This module aims to help adapt various common encoding types and implementations into one unified interface.

use serde::{Deserialize, Serialize};
use std::fmt::Display;

use dev_disp_core::host::VirtualScreenPixelFormat;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RawParameters {
    pub pixel_format: VirtualScreenPixelFormat,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HevcParameters {
    pub profile: u8,
    pub compatibility: u8,
    pub level: u8,
    /// TODO: See if we can narrow this to an enum instead of a String
    pub tier: char,
    pub constraints: u8,
}

impl Default for HevcParameters {
    fn default() -> Self {
        // Need to research, understand, and better document these defaults
        HevcParameters {
            profile: 1,
            compatibility: 0x06,
            level: 93,
            tier: 'L',
            constraints: 0xB0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct H264Parameters {
    pub profile: u8,
    pub level: u8,
    pub constraint_flags: u8,
}

impl Default for H264Parameters {
    fn default() -> Self {
        // Need to research, understand, and better document these defaults
        H264Parameters {
            profile: 66,
            level: 30,
            constraint_flags: 0x00,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vp09Parameters {
    pub profile: u8,
    pub level: u8,
    pub bit_depth: u8,
}

impl Default for Vp09Parameters {
    fn default() -> Self {
        // These defaults depend on the pixel format, so they are definitely not sane to use universally
        Vp09Parameters {
            profile: 1,
            level: 10,
            bit_depth: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Av1Parameters {
    pub profile: u8,
    pub level: u8,
    pub constraint_flags: u8,
    pub bit_depth: u8,
}

impl Default for Av1Parameters {
    fn default() -> Self {
        // Need to research, understand, and better document these defaults
        Av1Parameters {
            profile: 66,
            level: 30,
            constraint_flags: 0x00,
            bit_depth: 8,
        }
    }
}

// Well-known defined encoder families we can negotiate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Codec {
    Raw(RawParameters),
    Hevc(HevcParameters),
    H264(H264Parameters),
    Vp09(Vp09Parameters),
    Vp8, // TODO: Any VP8 parameters?
    Av1(Av1Parameters),
}

impl Default for Codec {
    fn default() -> Self {
        Codec::Raw(RawParameters::default())
    }
}

impl Display for Codec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Codec::Raw(_) => write!(f, "raw"),
            Codec::Hevc(_) => write!(f, "hevc"),
            Codec::H264(_) => write!(f, "h264"),
            Codec::Vp09(_) => write!(f, "vp09"),
            Codec::Vp8 => write!(f, "vp8"),
            Codec::Av1(_) => write!(f, "av1"),
        }
    }
}

impl Codec {
    pub fn family(&self) -> CodecFamily {
        match self {
            Codec::Raw(_) => CodecFamily::Raw,
            Codec::Hevc(_) => CodecFamily::Hevc,
            Codec::H264(_) => CodecFamily::H264,
            Codec::Vp09(_) => CodecFamily::Vp09,
            Codec::Vp8 => CodecFamily::Vp8,
            Codec::Av1(_) => CodecFamily::Av1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CodecFamily {
    #[default]
    Raw,
    Hevc,
    H264,
    Vp09,
    Vp8,
    Av1,
}

impl Display for CodecFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecFamily::Raw => write!(f, "raw"),
            CodecFamily::Hevc => write!(f, "hevc"),
            CodecFamily::H264 => write!(f, "h264"),
            CodecFamily::Vp09 => write!(f, "vp09"),
            CodecFamily::Vp8 => write!(f, "vp8"),
            CodecFamily::Av1 => write!(f, "av1"),
        }
    }
}
