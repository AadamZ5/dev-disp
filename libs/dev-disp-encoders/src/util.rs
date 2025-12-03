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
        VirtualScreenPixelFormat::Argb8888 => ffmpeg::format::Pixel::ARGB,

        // EVDI is reporting ABGR, but really giving us BGRA data
        // TODO: Investigate and fix!
        VirtualScreenPixelFormat::Abgr8888 => ffmpeg::format::Pixel::RGBA,
    }
}
