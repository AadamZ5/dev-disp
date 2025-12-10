use crate::{Edid, EdidDisplayParameters, EdidDpmsFeatures};

pub enum EdidBuilderError {
    InvalidManufacturerId,
}

pub struct EdidBuilder {
    edid: Edid,
}

impl EdidBuilder {
    pub fn new() -> Self {
        Self {
            edid: Edid::default(),
        }
    }

    pub fn manufacturer_id(mut self, manufacturer_id: &str) -> Self {
        self.edid.manufacturer_id = manufacturer_id.to_string();
        self
    }

    pub fn product_code(mut self, product_code: u16) -> Self {
        self.edid.product_code = product_code;
        self
    }

    pub fn serial(mut self, serial: u32) -> Self {
        self.edid.serial = serial;
        self
    }

    pub fn version_week(mut self, week_of_year: u8) -> Self {
        self.edid.version_week = week_of_year;
        self
    }

    pub fn version_year(mut self, version_year: u8) -> Self {
        self.edid.version_year = version_year;
        self
    }

    pub fn version_major(mut self, edid_major_version: u8) -> Self {
        self.edid.version_edid = edid_major_version;
        self
    }

    pub fn version_minor(mut self, edid_minor_version: u8) -> Self {
        self.edid.version_rev = edid_minor_version;
        self
    }

    pub fn display_parameters(mut self, display_params: EdidDisplayParameters) -> Self {
        self.edid.display_parameters = display_params;
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: (u8, u8)) -> Self {
        let aspect_ratio_value = (aspect_ratio.0 as f32) / (aspect_ratio.1 as f32);
        let byte_value = (aspect_ratio_value * 100.0).round() as u8 - 99;

        // Setting landscape aspect ratio will set height to 0, and set
        // width to the computed aspect ratio byte value.
        self.edid.width = byte_value;
        self.edid.height = 0;
        self
    }

    pub fn dpms_features(mut self, dpms_features: EdidDpmsFeatures) -> Self {
        self.edid.dpms_features = dpms_features;
        self
    }
}

impl EdidBuilder {
    pub fn build(self) -> Result<Edid, EdidBuilderError> {
        // TODO: Perform validation on all fields and value
        // ranges! See https://en.wikipedia.org/wiki/Extended_Display_Identification_Data
        Ok(self.edid)
    }
}
