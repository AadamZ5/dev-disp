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
        VirtualScreenPixelFormat::Abgr8888 => ffmpeg::format::Pixel::ABGR,

        // TODO: VP9 scaler/encoder seems to have issues with ARGB input? Make
        // TODO: a "quirk" for specifically VP9 encoders that interpret ARGB as
        // TODO: BGRA instead
        VirtualScreenPixelFormat::Argb8888 => ffmpeg::format::Pixel::BGRA,
    }
}
