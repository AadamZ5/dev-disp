use crate::{descriptors::EdidDescriptor, edid};

#[derive(Debug, Clone, Copy, Default)]
pub enum EdidDigitalBitDepth {
    Undefined = 0b000,
    Six = 0b001,
    #[default]
    Eight = 0b010,
    Ten = 0b011,
    Twelve = 0b100,
    Fourteen = 0b101,
    Sixteen = 0b110,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum EdidDigitalVideoInterface {
    Undefined = 0b0000,
    DVI = 0b0001,
    HDMIa = 0b0010,
    HDMIb = 0b0011,
    MDDI = 0b0100,
    #[default]
    DisplayPort = 0b0101,
}

#[derive(Debug, Clone, Copy)]
pub enum EdidDisplayParameters {
    Digital((EdidDigitalBitDepth, EdidDigitalVideoInterface)),
    // TODO: Analog!
}

impl Default for EdidDisplayParameters {
    fn default() -> Self {
        EdidDisplayParameters::Digital((
            EdidDigitalBitDepth::default(),
            EdidDigitalVideoInterface::default(),
        ))
    }
}

// TODO: Better as bit flags
#[derive(Debug, Clone, Copy, Default)]
pub enum EdidDpmsDigitalDisplayType {
    Rgb444 = 0b00,
    Rgb444YCrCb444 = 0b01,
    RGB444YCrCb422 = 0b10,
    #[default]
    RGB444YCrCb444YCrCb422 = 0b11,
}

#[derive(Debug, Clone, Default)]
pub enum EdidDpmsAnalogDisplayType {
    MonochromeOrGrayscale = 0b00,
    #[default]
    RgbColor = 0b01,
    NonRgbColor = 0b10,
    Undefined = 0b11,
}

#[derive(Debug, Clone)]
pub enum EdidDpmsDisplayType {
    Digital(EdidDpmsDigitalDisplayType),
    Analog(EdidDpmsAnalogDisplayType),
}

impl Default for EdidDpmsDisplayType {
    fn default() -> Self {
        EdidDpmsDisplayType::Digital(EdidDpmsDigitalDisplayType::default())
    }
}

#[derive(Debug, Clone, Default)]
pub struct EdidDpmsFeatures {
    /// True if the display supports DPMS standby mode
    pub standby: bool,
    /// True if the display supports DPMS suspend mode
    pub suspend: bool,
    /// True if the display supports DPMS active-off mode
    pub active_off: bool,
    /// The type of display (digital or analog) and its specification
    pub display_type: EdidDpmsDisplayType,
    /// True if the display supports sRGB standard color space
    pub srgb_standard: bool,
    /// True if the preferred timing mode is specified in the first detailed timing descriptor
    pub preferred_timing_mode: bool,
    /// True if the display supports continuous frequencies
    pub continuous_frequency: bool,
}

impl EdidDpmsFeatures {
    pub fn to_byte(&self) -> u8 {
        let mut byte_value: u8 = 0;

        if self.standby {
            byte_value |= 0b1000_0000;
        }
        if self.suspend {
            byte_value |= 0b0100_0000;
        }
        if self.active_off {
            byte_value |= 0b0010_0000;
        }

        let display_type_bits: u8 = match self.display_type {
            EdidDpmsDisplayType::Digital(ref dt) => (dt.clone() as u8) << 3,
            EdidDpmsDisplayType::Analog(ref at) => (at.clone() as u8) << 3,
        };
        byte_value |= display_type_bits;

        if self.srgb_standard {
            byte_value |= 0b0000_0100;
        }
        if self.preferred_timing_mode {
            byte_value |= 0b0000_0010;
        }
        if self.continuous_frequency {
            byte_value |= 0b0000_0001;
        }

        byte_value
    }
}

impl From<EdidDpmsFeatures> for u8 {
    fn from(features: EdidDpmsFeatures) -> u8 {
        features.to_byte()
    }
}

#[derive(Debug, Clone, Default, Copy)]
pub struct EdidEstablishedTimingSupport {
    pub t720x400_70hz: bool,
    pub t720x400_88hz: bool,
    pub t640x480_60hz: bool,
    pub t640x480_67hz: bool,
    pub t640x480_72hz: bool,
    pub t640x480_75hz: bool,
    pub t800x600_56hz: bool,
    pub t800x600_60hz: bool,
    pub t800x600_72hz: bool,
    pub t800x600_75hz: bool,
    pub t832x624_75hz: bool,
    pub t1024x768_87hz: bool,
    pub t1024x768_60hz: bool,
    pub t1024x768_70hz: bool,
    pub t1024x768_75hz: bool,
    pub t1280x1024_75hz: bool,
    pub t1152x870_75hz: bool,
}

impl EdidEstablishedTimingSupport {
    pub fn to_bytes(&self) -> [u8; 3] {
        let mut bytes = [0u8; 3];

        if self.t720x400_70hz {
            bytes[0] |= 0b1000_0000;
        }
        if self.t720x400_88hz {
            bytes[0] |= 0b0100_0000;
        }
        if self.t640x480_60hz {
            bytes[0] |= 0b0010_0000;
        }
        if self.t640x480_67hz {
            bytes[0] |= 0b0001_0000;
        }
        if self.t640x480_72hz {
            bytes[0] |= 0b0000_1000;
        }
        if self.t640x480_75hz {
            bytes[0] |= 0b0000_0100;
        }
        if self.t800x600_56hz {
            bytes[0] |= 0b0000_0010;
        }
        if self.t800x600_60hz {
            bytes[0] |= 0b0000_0001;
        }

        if self.t800x600_72hz {
            bytes[1] |= 0b1000_0000;
        }
        if self.t800x600_75hz {
            bytes[1] |= 0b0100_0000;
        }
        if self.t832x624_75hz {
            bytes[1] |= 0b0010_0000;
        }
        if self.t1024x768_87hz {
            bytes[1] |= 0b0001_0000;
        }
        if self.t1024x768_60hz {
            bytes[1] |= 0b0000_1000;
        }
        if self.t1024x768_70hz {
            bytes[1] |= 0b0000_0100;
        }
        if self.t1024x768_75hz {
            bytes[1] |= 0b0000_0010;
        }
        if self.t1280x1024_75hz {
            bytes[1] |= 0b0000_0001;
        }

        if self.t1152x870_75hz {
            bytes[2] |= 0b1000_0000;
        }

        bytes
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum EdidStandardTimingAspectRatio {
    #[default]
    Ar16_10 = 0b00,
    Ar4_3 = 0b01,
    Ar5_4 = 0b10,
    Ar16_9 = 0b11,
}

#[derive(Debug, Clone, Default)]
pub struct EdidStandardTiming {
    /// Horizontal resolution byte value, will be transformed
    /// from pixels per the EDID spec as follows:
    /// ```rust
    /// let horizontal_resolution: u16 = 1024;
    /// let byte_value = (horizontal_resolution / 8) - 31 as u8;
    /// ```
    pub horizontal_resolution: u16,

    /// Standard aspect ratio
    pub aspect_ratio: EdidStandardTimingAspectRatio,

    /// Vertical frequency byte value, will be transformed from
    /// Hz per the EDID spec as follows:
    /// ```rust
    /// let refresh_rate: u8 = 60;
    /// let byte_value = refresh_rate - 60;
    /// ```
    pub refresh_rate: u8,
}

impl EdidStandardTiming {
    pub fn to_bytes(&self) -> [u8; 2] {
        let mut bytes = [0u8; 2];

        bytes[0] = ((self.horizontal_resolution / 8)
            .checked_sub(31)
            .unwrap_or(0)) as u8;

        let aspect_ratio_bits = (self.aspect_ratio as u8) << 6;
        let refresh_rate_bits = (self.refresh_rate - 60) & 0b0011_1111;

        bytes[1] = aspect_ratio_bits | refresh_rate_bits;

        bytes
    }
}

#[derive(Debug, Clone)]
pub struct Edid {
    /// A 3 character manufacturer ID
    pub manufacturer_id: String,

    /// A product code, usually represented as a 16-bit number / 4-digit hex
    pub product_code: u16,

    /// A 4 byte serial number
    pub serial: u32,

    /// Week of year (or 0 if not specified)
    pub version_week: u8,
    /// Year offset from 1990 (e.g., 20 = 2010)
    pub version_year: u8,
    /// EDID version (major) (typically 1)
    pub version_edid: u8,
    /// EDID revision (minor) (typically 3 or 4)
    pub version_rev: u8,

    /// Digital or analog display parameters
    pub display_parameters: EdidDisplayParameters,

    /// Width in CM or horizontal aspect ratio value.
    ///
    /// To specify the horizontal aspect ratio, the height value
    /// should be set to 0, and the aspect ratio needs transformed
    /// as per EDID spec:
    /// ```rust
    /// let aspect_ratio_value = (aspect_ratio.0 as f32) / (aspect_ratio.1 as f32);
    /// let byte_value = (aspect_ratio_value * 100.0).round() as u8 - 99;
    /// ```
    /// TODO: Consider changing to enum to differentiate between aspect
    /// ratio and physical size in cm.
    pub width: u8,
    pub height: u8,

    /// Gamma correction factor (???)
    pub gamma: u8,

    /// DPMS feature flags
    pub dpms_features: EdidDpmsFeatures,

    /// 10 bytes defining chromaticity coordinates
    /// Refer to CIE 1931
    /// Red, Green, Blue primary coordinates + White point coordinates
    /// Idk what is going on here.
    /// TODO: Define a struct that better supports this data
    pub color_characteristics: [u8; 10],

    /// Legacy timing options supported
    pub timing_support_flags: EdidEstablishedTimingSupport,

    /// 8 pairs of 2-byte timing definitions
    ///
    /// Each timing uses 2 bytes (16 entries total),
    /// Encodes horizontal resolution and refresh rate,
    /// Aspect ratio derived from EDID version,
    /// Value 0x0101 indicates unused slot
    pub standard_timings: [Option<EdidStandardTiming>; 8],

    pub descriptor_1: Option<EdidDescriptor>,
    pub descriptor_2: Option<EdidDescriptor>,
    pub descriptor_3: Option<EdidDescriptor>,
    pub descriptor_4: Option<EdidDescriptor>,
}

/// Default Manufacturer ID for Undetermined Displays
/// See: https://uefi.org/PNP_ID_List
const DEV_DISP_EDID_MANUFACTURER_ID: &str = "UND";

/// Converts a 3 character manufacturer ID into its 3 byte EDID representation.
///
/// Each letter of the alphabet is represented as a 5 bit value, where the value
/// is the position of the letter in the alphabet.
/// For example, 'A' = 1, 'B' = 2, ..., 'Z' = 26.
///
/// The returned bytes is 2 bytes of 3 5-bit character values.
///
/// Example:
/// ```text
///    | CH1  |   CH2  |  CH3 |
///    |      |        |      |
///  |0 00001   00 010   00011|
///  |            |           |
///  |   Byte 1   |   Byte 2  |
/// ```
/// This layout above produces the manufacturer code of `"ABC"` in just two bytes.
///
fn manufacturer_id_to_bytes(manufacturer_id: &str) -> Result<[u8; 2], String> {
    if manufacturer_id.len() != 3 {
        return Err("Manufacturer ID must be exactly 3 characters".to_string());
    }

    let mut mfr_id_value = 0 as u16;

    let a_value = 'A' as usize;
    let z_value = 'Z' as usize;
    for (i, c) in manufacturer_id.chars().enumerate() {
        let c_value = c as usize;
        if c_value < a_value || c_value > z_value {
            return Err(format!(
                "Character '{}' ({}) at position {} is out of the encodable range!",
                c, c_value, i
            ));
        }

        // Alphabet position is a 1-based index
        let alphabet_position = ((c_value - a_value) + 1) as u16;

        // This should be a 5-bit number.
        // The value desired is a 2-byte BE value of 3 5-bit character values

        // We should shift the bits by i*5 and add it to the 2 byte value
        // We add `+1` here because there should be a 0 at the beginning
        // of the 2-byte value.
        // 0 01001 00010 01101
        let shifted_position_value = alphabet_position << ((2 - i) * 5);
        mfr_id_value |= shifted_position_value;
    }

    let bytes: [u8; 2] = [(mfr_id_value << 0) as u8, (mfr_id_value << 8) as u8];

    Ok(bytes)
}

impl Edid {
    /// The EDID header sequence
    const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mfr_bytes = manufacturer_id_to_bytes(&self.manufacturer_id)?;

        let mut edid_bytes: Vec<u8> = Vec::with_capacity(128);
        edid_bytes.resize(128, 0);

        // TODO: Assert sizes or use index-getting methods that
        // can return `Err()` on out-of-bounds access. The below
        // is bad because we are trusting each iter to be the
        // correct size.
        for (i, b) in Self::HEADER.iter().enumerate() {
            edid_bytes[i + 0] = *b;
        }

        for (i, b) in mfr_bytes.iter().enumerate() {
            edid_bytes[i + 8] = *b;
        }

        for (i, b) in self.product_code.to_le_bytes().iter().enumerate() {
            edid_bytes[i + 10] = *b;
        }

        for (i, b) in self.serial.to_le_bytes().iter().enumerate() {
            edid_bytes[i + 12] = *b;
        }

        edid_bytes[16] = self.version_week;
        edid_bytes[17] = self.version_year;
        edid_bytes[18] = self.version_edid;
        edid_bytes[19] = self.version_rev;

        let basic_display_param_byte = match self.display_parameters {
            EdidDisplayParameters::Digital((ref bit_depth, ref params)) => {
                let mut param_byte = 0b10000000;
                let bit_depth_bytes = (*bit_depth as u8) << 4;
                param_byte |= bit_depth_bytes;
                param_byte |= *params as u8;
                param_byte
            }
        };

        edid_bytes[20] = basic_display_param_byte;

        edid_bytes[21] = self.width;
        edid_bytes[22] = self.height;
        // Note: Gamma is stored as (realGamma * 100) - 100
        edid_bytes[23] = self.gamma;
        edid_bytes[24] = self.dpms_features.to_byte();

        for (i, b) in self.color_characteristics.iter().enumerate() {
            edid_bytes[25 + i] = *b;
        }

        for (i, b) in self.timing_support_flags.to_bytes().iter().enumerate() {
            edid_bytes[35 + i] = *b;
        }

        let no_standard_timing = [0x01, 0x01];
        for (i, timing) in self.standard_timings.iter().enumerate() {
            for (j, b) in timing
                .as_ref()
                .map(|st| st.to_bytes())
                .unwrap_or(no_standard_timing)
                .iter()
                .enumerate()
            {
                edid_bytes[38 + i * 2 + j] = *b;
            }
        }

        for (i, descriptor) in [
            &self.descriptor_1,
            &self.descriptor_2,
            &self.descriptor_3,
            &self.descriptor_4,
        ]
        .iter()
        .enumerate()
        {
            if let Some(desc) = descriptor {
                let desc_bytes = desc.to_bytes();
                // TODO: Verify byte-lengths
                for (j, b) in desc_bytes.iter().enumerate() {
                    edid_bytes[54 + (i * 18) + j] = *b;
                }
            }
        }

        // Extension flag
        edid_bytes[126] = 0;

        // Checksum
        let sum: u8 = edid_bytes[0..127]
            .iter()
            .fold(0, |acc, &x| acc.wrapping_add(x));
        edid_bytes[127] = (0u8).wrapping_sub(sum);

        // TODO: Implement EDID serialization
        // edid_bytes.copy_from_slice(TEMPLATE_EDID);
        Ok(edid_bytes)
    }
}

impl Default for Edid {
    fn default() -> Self {
        Edid {
            manufacturer_id: DEV_DISP_EDID_MANUFACTURER_ID.to_string(),
            product_code: 0x0000,
            serial: 0x0000,
            version_week: 0,
            version_year: 0,
            version_edid: 1,
            version_rev: 4,
            display_parameters: EdidDisplayParameters::default(),
            width: 78, // (16/9 - 1) * 100 = 78, per aspect ratio encoding
            height: 0,
            gamma: 0,
            dpms_features: EdidDpmsFeatures::default(),
            color_characteristics: [0; 10],
            timing_support_flags: EdidEstablishedTimingSupport::default(),
            standard_timings: [None, None, None, None, None, None, None, None],
            descriptor_1: None,
            descriptor_2: None,
            descriptor_3: None,
            descriptor_4: None,
        }
    }
}
