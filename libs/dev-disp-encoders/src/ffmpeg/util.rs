use dev_disp_core::host::VirtualScreenPixelFormat;
use ffmpeg_next as ffmpeg;

pub fn ffmpeg_format_from_internal_format(
    format: &VirtualScreenPixelFormat,
) -> ffmpeg::format::Pixel {
    match format {
        VirtualScreenPixelFormat::Rgb888 => ffmpeg::format::Pixel::RGB24,
        VirtualScreenPixelFormat::Bgr888 => ffmpeg::format::Pixel::BGR24,
        VirtualScreenPixelFormat::Rgba8888 => ffmpeg::format::Pixel::RGBA,
        VirtualScreenPixelFormat::Bgra8888 => ffmpeg::format::Pixel::BGRA,

        // TODO: VP9 scaler/encoder seems to have issues with ARGB, and/or ABGR input? Make
        // TODO: a "quirk" for specifically VP9 encoders that interpret ARGB as
        // TODO: BGRA instead
        VirtualScreenPixelFormat::Argb8888 => ffmpeg::format::Pixel::BGRA,
        VirtualScreenPixelFormat::Abgr8888 => ffmpeg::format::Pixel::RGBA,
    }
}

pub fn internal_format_from_ffmpeg_format(
    format: &ffmpeg::format::Pixel,
) -> Option<VirtualScreenPixelFormat> {
    match format {
        ffmpeg::format::Pixel::RGB24 => Some(VirtualScreenPixelFormat::Rgb888),
        ffmpeg::format::Pixel::BGR24 => Some(VirtualScreenPixelFormat::Bgr888),
        ffmpeg::format::Pixel::RGBA => Some(VirtualScreenPixelFormat::Rgba8888),
        ffmpeg::format::Pixel::BGRA => Some(VirtualScreenPixelFormat::Bgra8888),
        _ => None,
    }
}
