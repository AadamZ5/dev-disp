#[derive(Debug, Clone, Copy, Default)]
pub enum SignalInterfaceType {
    #[default]
    NonInterlaced = 0,
    Interlaced = 1,
}

// TODO: This may be better represented as bitflags
#[derive(Debug, Clone, Copy, Default)]
pub enum StereoMode {
    /// No stereo timing mode
    /// (bit 0 is don't care)
    #[default]
    None = 0b000,

    FieldSequentialRightStereoSync = 0b010,
    FieldSequentialLeftStereoSync = 0b100,
    BiInterleavedRightImageEvenLines = 0b011,
    BiInterleavedLeftImageEvenLines = 0b101,
    QuadInterleaved = 0b110,
    SideBySideInterleaved = 0b111,
}

#[derive(Debug, Clone, Default)]
pub struct AnalogSyncFlags {
    /// Composite
    ///  - `true` = bipolar composite
    ///  - `false` = analog composite
    pub bipolar_analog_composite: bool,

    /// Serration
    ///  - `true` = serration pulse required for interlaced sync (H-sync during V-sync)
    ///  - `false` = no serrations
    pub serration: bool,

    /// Sync behavior
    ///  - `true` = sync on all RGB channels
    ///  - `false` = sync on green channel only
    pub sync_all: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DigitalSyncCompositeFlags {
    /// Serration
    /// - `true` = serration pulse required for interlaced sync (H-sync during V-sync)
    /// - `false` = no serrations
    pub serration: bool,

    /// H-sync polarity
    ///  - `true` = positive polarity
    ///  - `false` = negative polarity
    pub h_sync_positive: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DigitalSyncFlags {
    /// V-sync polarity
    /// - `true` = positive polarity
    /// - `false` = negative polarity
    pub v_sync_polarity: bool,

    /// H-sync polarity
    ///  - `true` = positive polarity
    ///  - `false` = negative polarity
    pub h_sync_positive: bool,
}

#[derive(Debug, Clone)]
pub enum SyncType {
    Analog(AnalogSyncFlags),
    DigitalComposite(DigitalSyncCompositeFlags),
    Digital(DigitalSyncFlags),
}

impl Default for SyncType {
    fn default() -> Self {
        SyncType::Digital(DigitalSyncFlags::default())
    }
}

#[derive(Debug, Clone, Default)]
pub struct FeaturesMap {
    pub signal_type: SignalInterfaceType,
    pub stereo_mode: StereoMode,
    pub sync_type: SyncType,
}

impl FeaturesMap {
    pub fn to_byte(&self) -> u8 {
        let mut byte_value: u8 = 0b0000_0000;

        byte_value |= (self.signal_type as u8) << 7;

        // This is the weirdest fucking setup for packing bytes but whatever.
        // Bit 1 and 2 of the stereo go into bits 6 and 5 of the byte,
        // but bit 0 of the stereo_mode goes into bit 0 of the byte.
        let mut stereo_mode_byte = ((self.stereo_mode as u8) << 4) & 0b0110_0000;
        stereo_mode_byte |= (self.stereo_mode as u8) & 0b0000_0001;
        byte_value |= stereo_mode_byte;

        let mut sync_bits = 0b0000_0000;
        match &self.sync_type {
            SyncType::Analog(flags) => {
                sync_bits |= 0b0000_0000; // Bit 4 is a zero for analog
                if flags.bipolar_analog_composite {
                    byte_value |= 1 << 3;
                }
                if flags.serration {
                    byte_value |= 1 << 2;
                }
                if flags.sync_all {
                    byte_value |= 1 << 1;
                }
            }
            SyncType::DigitalComposite(flags) => {
                sync_bits |= 0b0001_1000; // Bit 4 and 3 are one for digital
                if flags.serration {
                    sync_bits |= 1 << 2;
                }
                if flags.h_sync_positive {
                    sync_bits |= 1 << 1;
                }
            }
            SyncType::Digital(flags) => {
                sync_bits |= 0b0001_0000; // Bit 4 is one and bit 3 is zero for digital separate
                if flags.v_sync_polarity {
                    sync_bits |= 1 << 2;
                }
                if flags.h_sync_positive {
                    sync_bits |= 1 << 1;
                }
            }
        }
        byte_value |= sync_bits;

        // Bit 1 is already used with stereo_mode bytes above

        byte_value
    }
}

#[derive(Debug, Clone)]
pub struct EdidDetailedTimingDescriptor {
    /// Pixel clock. `00` = reserved; otherwise in 10 kHz units (0.01–655.35 MHz, little-endian).
    pub pixel_clock: u16,

    /// Horizontal active pixels, max 12-bit value
    /// Values will be truncated to a 12-bit value.
    pub horizontal_active_pixels: u16,

    /// Horizontal blanking pixels, end of active pixels to
    /// start of the next line. Max 12-bit value.
    /// Values will be truncated to a 12-bit value.
    pub horizontal_blanking_pixels: u16,

    /// Vertical active lines per frame, max 12-bit value
    /// Values will be truncated to a 12-bit value.
    pub vertical_active_lines: u16,

    /// Vertical blanking lines per frame, end of active lines
    /// to start of the next frame. Max 12-bit value.
    /// Values will be truncated to a 12-bit value.
    pub vertical_blanking_lines: u16,

    /// Horizontal sync offset (front porch), in pixels
    /// Max 12-bit value.
    /// Values will be truncated to a 10-bit value.
    pub horizontal_sync_offset: u16,

    /// Horizontal sync pulse width, in pixels
    /// Max 12-bit value.
    /// Values will be truncated to a 10-bit value.
    pub horizontal_sync_pulse_width: u16,

    /// Vertical sync offset (front porch), in lines
    /// Truncated to a 6-bit value.
    pub vertical_sync_offset: u8,

    /// Vertical sync pulse width, in lines
    /// Truncated to a 6-bit value.
    pub vertical_sync_pulse_width: u8,

    /// Horizontal image size, in millimeters
    /// Truncated to a 12-bit value.
    pub horizontal_image_size_mm: u16,

    /// Vertical image size, in millimeters
    /// Truncated to a 12-bit value.
    pub vertical_image_size_mm: u16,

    /// Horizontal border, in pixels (one side)
    /// Truly an 8-bit value.
    pub horizontal_border: u8,

    /// Vertical border, in lines (one side)
    /// Truly an 8-bit value.
    pub vertical_border: u8,

    /// Timing features map,
    /// 1 byte representing various features.
    pub features: FeaturesMap,
}

impl EdidDetailedTimingDescriptor {
    pub fn to_bytes(&self) -> [u8; 18] {
        let mut bytes = [0u8; 18];

        let pixel_clock_le = self.pixel_clock.to_le_bytes();
        bytes[0] = pixel_clock_le[0];
        bytes[1] = pixel_clock_le[1];

        let h_active_lsb = (self.horizontal_active_pixels & 0xFF) as u8;
        let h_blanking_lsb = (self.horizontal_blanking_pixels & 0xFF) as u8;
        bytes[2] = h_active_lsb;
        bytes[3] = h_blanking_lsb;

        // We pack the 4 most significant bits of horizontal active and blanking
        // into a single byte. Idk why EDID does this but whatever.
        let h_active_msb = ((self.horizontal_active_pixels >> 8) & 0x0F) as u8;
        let h_blanking_msb = ((self.horizontal_blanking_pixels >> 8) & 0x0F) as u8;
        bytes[4] = (h_active_msb << 4) | h_blanking_msb;

        let v_active_msb = (self.vertical_active_lines & 0xFF) as u8;
        let v_blanking_msb = (self.vertical_blanking_lines & 0xFF) as u8;
        bytes[5] = v_active_msb;
        bytes[6] = v_blanking_msb;

        // Same again, packing 4 most significant bits of vertical active and blanking
        // into a single byte.
        let v_active_msb = ((self.vertical_active_lines >> 8) & 0x0F) as u8;
        let v_blanking_msb = ((self.vertical_blanking_lines >> 8) & 0x0F) as u8;
        bytes[7] = (v_active_msb << 4) | v_blanking_msb;

        let h_sync_offset_lsb = (self.horizontal_sync_offset & 0xFF) as u8;
        let h_sync_pulse_width_lsb = (self.horizontal_sync_pulse_width & 0xFF) as u8;
        bytes[8] = h_sync_offset_lsb;
        bytes[9] = h_sync_pulse_width_lsb;

        // Now, packing 4 lsb of vertical sync offset and pulse into a single byte.
        // This is because vsync offset and pulse are u6 values instead of u12 like
        // the horizontal values.
        let v_sync_offset_lsb = self.vertical_sync_offset & 0x0F;
        let v_sync_pulse_width_lsb = self.vertical_sync_pulse_width & 0x0F;
        bytes[10] = (v_sync_offset_lsb << 4) | v_sync_pulse_width_lsb;

        // We're only taking the 2 msb of each value here, all of these get packed into
        // one byte (which is kind of crazy).
        let h_sync_offset_msb = ((self.horizontal_sync_offset >> 8) & 0b11) as u8;
        let h_sync_pulse_width_msb = ((self.horizontal_sync_pulse_width >> 8) & 0b11) as u8;
        let v_sync_offset_msb = (self.vertical_sync_offset >> 4) & 0b11;
        let v_sync_pulse_width_msb = (self.vertical_sync_pulse_width >> 4) & 0b11;
        bytes[11] = (h_sync_offset_msb << 6)
            | (h_sync_pulse_width_msb << 4)
            | (v_sync_offset_msb << 2)
            | v_sync_pulse_width_msb;

        let h_image_size_lsb = (self.horizontal_image_size_mm & 0xFF) as u8;
        let v_image_size_lsb = (self.vertical_image_size_mm & 0xFF) as u8;
        bytes[12] = h_image_size_lsb;
        bytes[13] = v_image_size_lsb;

        // Packing 4 msb of horizontal and vertical image size into a single byte.
        let h_image_size_msb = ((self.horizontal_image_size_mm >> 8) & 0x0F) as u8;
        let v_image_size_msb = ((self.vertical_image_size_mm >> 8) & 0x0F) as u8;
        bytes[14] = (h_image_size_msb << 4) | v_image_size_msb;

        bytes[15] = self.horizontal_border;
        bytes[16] = self.vertical_border;
        bytes[17] = self.features.to_byte();

        bytes
    }
}
