import { VIDEO_CODEC_DEFINITIONS, VideoCodecId } from './lib/video';
import { CodecParameterStringFn } from './lib/video/common';

/**
 * Returns true if the given codec string is one defined by this library.
 *
 * @see {@link VIDEO_CODEC_DEFINITIONS}
 *
 * @param codec The codec to test
 * @returns true if the codec is defined in this library
 */
export function isDefinedVideoCodec<T extends string>(
  codec: string
): codec is VideoCodecId {
  return VIDEO_CODEC_DEFINITIONS.some((def) => def.codec === codec);
}

export type SearchCodecResult = {
  support: VideoDecoderSupport;
  definition: (typeof VIDEO_CODEC_DEFINITIONS)[number];
};

/**
 * Given a codec name and a set of parameters, search through the known video codec
 * definitions and return those that are supported by the current environment.
 *
 * @param codec The *WEB* codec name, e.g., 'av01', 'vp09', etc. (See {@link VideoCodecId})
 * @param parameters The codec-specific parameters to test compatibility against
 * @returns A list of supported codec definitions matching the given codec and parameters
 */
export async function searchSupportedVideoDecoders(
  codec: string,
  parameters?: Record<string, string | number>
): Promise<SearchCodecResult[]> {
  if (!('VideoDecoder' in window)) {
    console.warn('VideoDecoder is not supported in this environment');
    return [];
  }

  const supportResults = await Promise.allSettled(
    // Filter first, find the codecs that match the ID given
    VIDEO_CODEC_DEFINITIONS.filter((def) => def.codec === codec).map(
      async (definition) => {
        const paramString = (
          definition.toParamString as CodecParameterStringFn
        )(definition.codec, parameters ?? null);
        const fullCodecString = paramString;

        const support = await VideoDecoder.isConfigSupported({
          codec: fullCodecString,
        });
        return { support, definition };
      }
    )
  );

  const supportedCodecs = supportResults
    .filter(
      (result): result is PromiseFulfilledResult<SearchCodecResult> =>
        result.status === 'fulfilled' && result.value.support.supported === true
    )
    .map((result) => result.value);

  return supportedCodecs;
}
