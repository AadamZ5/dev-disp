use dev_disp_core::host::DisplayParameters;

use crate::edid;

#[derive(Debug, Clone)]
pub struct EdidDetailedTimingDescriptor {
    /// Pixel clock. `00` = reserved; otherwise in 10 kHz units (0.01–655.35 MHz, little-endian).
    pixel_clock: u16,

    horizontal_active_pixels: u16,
}

#[derive(Debug, Clone)]
pub enum EdidDescriptor {
    DetailedTiming(EdidDetailedTimingDescriptor),
}

#[derive(Debug, Clone, Copy)]
pub enum EdidDigitalBitDepth {
    Undefined = 0b000,
    Six = 0b001,
    Eight = 0b010,
    Ten = 0b011,
    Twelve = 0b100,
    Fourteen = 0b101,
    Sixteen = 0b110,
}

#[derive(Debug, Clone, Copy)]
pub enum EdidDigitalVideoInterface {
    Undefined = 0b0000,
    DVI = 0b0001,
    HDMIa = 0b0010,
    HDMIb = 0b0011,
    MDDI = 0b0100,
    DisplayPort = 0b0101,
}

#[derive(Debug, Clone, Copy)]
pub enum EdidDisplayParameters {
    Digital((EdidDigitalBitDepth, EdidDigitalVideoInterface)),
    // TODO: Analog!
}

#[derive(Debug, Clone)]
pub struct Edid {
    /// A 3 character manufacturer ID
    manufacturer_id: String,

    /// A product code, usually represented as a 16-bit number / 4-digit hex
    product_code: u16,

    /// A 4 byte serial number
    serial: u32,

    /// Week of year (or 0 if not specified)
    version_week: u8,
    /// Year offset from 1990 (e.g., 20 = 2010)
    version_year: u8,
    /// EDID version (major) (typically 1)
    version_edid: u8,
    /// EDID revision (minor) (typically 3 or 4)
    version_rev: u8,

    /// Digital or analog display parameters
    display_parameters: EdidDisplayParameters,

    width_cm: u8,
    height_cm: u8,

    /// Gamma correction factor (???)
    gamma: u8,

    /// DPMS feature flags
    dpms_features: u8,

    /// 10 bytes defining chromaticity coordinates
    /// Refer to CIE 1931
    /// Red, Green, Blue primary coordinates + White point coordinates
    color_characteristics: [u8; 10],

    /// Legacy timing options supported
    ///
    /// TODO: Define an enum that can serialize to a `[u8; 3]`
    timing_support_flags: [u8; 3],

    /// 8 pairs of 2-byte timing definitions
    ///
    /// Each timing uses 2 bytes (16 entries total),
    /// Encodes horizontal resolution and refresh rate,
    /// Aspect ratio derived from EDID version,
    /// Value 0x0101 indicates unused slot
    standard_timings: [u8; 16],

    descriptor_1: Option<EdidDescriptor>,
    descriptor_2: Option<EdidDescriptor>,
    descriptor_3: Option<EdidDescriptor>,
    descriptor_4: Option<EdidDescriptor>,
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
/// The returned bytes is a 2-byte value of 3 5-bit character values.
///
/// Example:
/// ```text
///    | CH1  |   CH2  |  CH3 |
///    |      |        |      |
/// B  |00001   000 10   00011|
///    |           |          |
///    |  Byte 1   |  Byte 2  |
/// ```
/// This layout above produces the manufacturer code of `"ABC"` in just two bytes.
///
fn manufacturer_id_to_bytes(manufacturer_id: &str) -> Result<[u8; 2], String> {
    if manufacturer_id.len() != 3 {
        return Err("Manufacturer ID must be exactly 3 characters".to_string());
    }

    let mut mfr_id_value = 0 as u32;

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
        let alphabet_position = ((c_value - a_value) + 1) as u32;

        // This should be a 5-bit number.
        // The value desired is a 2-byte BE value of 3 5-bit character values

        // We should shift the bits by i*5 and add it to the 2 byte value
        // We add `+1` here because there should be a 0 at the beginning
        // of the 2-byte value.
        // 0 01001 00010 01101
        let in_place_value = alphabet_position >> (3 - i * 5) + 1;
        mfr_id_value |= in_place_value;
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

        for (i, b) in Self::HEADER.iter().enumerate() {
            edid_bytes[i] = *b;
        }

        let offset = 8;
        for (i, b) in mfr_bytes.iter().enumerate() {
            edid_bytes[i + offset] = *b;
        }

        let offset = 10;
        for (i, b) in self.product_code.to_be_bytes().iter().enumerate() {
            edid_bytes[i + offset] = *b;
        }

        let offset = 12;
        for (i, b) in self.serial.to_be_bytes().iter().enumerate() {
            edid_bytes[i + offset] = *b;
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

        // TODO: Implement EDID serialization
        edid_bytes.copy_from_slice(TEMPLATE_EDID);
        Ok(edid_bytes)
    }
}

impl Into<Edid> for DisplayParameters {
    fn into(self) -> Edid {
        Edid {
            ..Default::default()
        }
    }
}

const TEMPLATE_EDID: &[u8; 128] = include_bytes!("./Example_EDID.bin");
