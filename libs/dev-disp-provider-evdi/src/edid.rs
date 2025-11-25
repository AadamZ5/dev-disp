use dev_disp_core::host::DisplayParameters;

pub struct EdidDetailedTimingDescriptor {
    /// Pixel clock. `00` = reserved; otherwise in 10 kHz units (0.01–655.35 MHz, little-endian).
    pixel_clock: u16,

    horizontal_active_pixels: u16,
}

pub enum EdidDescriptor {}

pub struct Edid {
    /// A 3 character manufacturer ID
    manufacturer_id: String,

    /// A product code, usually represented in 4 digit hex
    product_code: u16,

    /// A 4 byte serial number
    serial: u32,

    /// Week of year (or 0 if not specified)
    version_week: u8,
    /// Year offset from 1990 (e.g., 20 = 2010)
    version_year: u8,
    /// EDID version (typically 1)
    version_edid: u8,
    /// EDID revision (typically 3 or 4)
    version_rev: u8,

    /// If the display is digital. If false, assume analog
    digital: bool,
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

impl Default for Edid {
    fn default() -> Self {
        Self {
            ..Default::default()
        }
    }
}

const TEMPLATE_EDID: &[u8; 128] = include_bytes!("./Example_EDID.bin");

/// Create an EDID buffer from the given display parameters.
///
/// TODO: Return an EDID structure instead of raw bytes!
pub fn edid_from_display_params(display_params: &DisplayParameters) -> [u8; 128] {
    let mut edid: [u8; 128] = [0; 128];
    edid.clone_from_slice(TEMPLATE_EDID);
    edid
}
