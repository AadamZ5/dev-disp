import { CodecDefinition } from './common';

/**
 * Parameter string:
 *   1. https://aomediacodec.github.io/av1-isobmff/#codecsparam
 *
 * ## From Section 5 of reference 1:
 * DASH and other applications require defined values
 * for the Codecs parameter specified in [RFC6381] for
 * ISO Media tracks. The codecs parameter string for
 * the AOM AV1 codec is as follows:
 *
 * ```text
 * <sample entry 4CC>.<profile>.<level><tier>.<bitDepth>.<monochrome>.<chromaSubsampling>.<colorPrimaries>.<transferCharacteristics>.<matrixCoefficients>.<videoFullRangeFlag>
 * ```
 *
 * All fields following the sample entry 4CC are expressed
 * as double digit decimals, unless indicated otherwise.
 * Leading or trailing zeros cannot be omitted.
 */
export type CodevAv1Parameters = {
  profile: number; // 0..2
  level: number; // 10..63
  tier: 'M' | 'H'; // Main or High
  bitDepth: number; // 8, 10, 12
  monochrome?: 0 | 1;
  chromaSubsampling?: number; // 0..3
  colorPrimaries?: number; // 0..255
  transferCharacteristics?: number; // 0..255
  matrixCoefficients?: number; // 0..255
  videoFullRangeFlag?: 0 | 1;
};

export function av1ToParamString(codec: 'av01', params: CodevAv1Parameters) {
  let acc = '' + codec;
  acc += `.${params.profile.toString().padStart(2, '0')}`;
  acc += `.${params.level.toString().padStart(2, '0')}${params.tier}`;
  acc += `.${params.bitDepth.toString().padStart(2, '0')}`;
  // If none of the optional parametrs are set, return early
  if (
    typeof params.monochrome !== 'number' &&
    typeof params.chromaSubsampling !== 'number' &&
    typeof params.colorPrimaries !== 'number' &&
    typeof params.transferCharacteristics !== 'number' &&
    typeof params.matrixCoefficients !== 'number' &&
    typeof params.videoFullRangeFlag !== 'number'
  ) {
    return acc;
  }

  // Defaults from bottom of section 5 of https://aomediacodec.github.io/av1-isobmff/#codecsparam
  acc += `.${params.monochrome ?? '0'}`;
  acc += `.${params.chromaSubsampling?.toString().padStart(2, '0') ?? '110'}`;
  acc += `.${params.colorPrimaries?.toString().padStart(2, '0') ?? '110'}`;
  acc += `.${
    params.transferCharacteristics?.toString().padStart(2, '0') ?? '01'
  }`;
  acc += `.${params.matrixCoefficients?.toString().padStart(2, '0') ?? '01'}`;
  acc += `.${params.videoFullRangeFlag ?? '0'}`;

  return acc;
}

/**
 * Overview: https://www.w3.org/TR/webcodecs-av1-codec-registration/
 *
 * @see {@link CodevAv1Parameters}
 */
export type CodecAv1 = CodecDefinition<CodevAv1Parameters, 'av01'>;

/**
 * @see {@link CodecAv1}
 */
export const CODEC_AV1: CodecAv1 = {
  codec: 'av01',
  displayName: 'AV1',
  toParamString: av1ToParamString,
};
