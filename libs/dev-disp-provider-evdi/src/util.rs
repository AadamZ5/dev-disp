use dev_disp_core::host::VirtualScreenPixelFormat;
use drm_fourcc::DrmFourcc;
use log::debug;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Unsupported FourCC format: {} (0x{:08X})", name, fourcc)]
pub struct UnsupportedFourccError {
    pub fourcc: u32,
    pub name: String,
}

pub fn evdi_format_to_internal_format(
    format: u32,
) -> Result<VirtualScreenPixelFormat, UnsupportedFourccError> {
    let result = DrmFourcc::try_from(format).map_err(|_| UnsupportedFourccError {
        fourcc: format,
        name: "????".to_string(),
    })?;

    match result {
        DrmFourcc::Rgba8888 => Ok(VirtualScreenPixelFormat::Rgba8888),
        DrmFourcc::Bgra8888 => Ok(VirtualScreenPixelFormat::Bgra8888),
        DrmFourcc::Argb8888 => Ok(VirtualScreenPixelFormat::Argb8888),
        DrmFourcc::Rgb888 => Ok(VirtualScreenPixelFormat::Rgb888),
        DrmFourcc::Bgr888 => Ok(VirtualScreenPixelFormat::Bgr888),
        DrmFourcc::Abgr8888 => Ok(VirtualScreenPixelFormat::Abgr8888),
        _ => Err(UnsupportedFourccError {
            fourcc: format,
            name: format!("{:?}", result),
        }),
    }
}
