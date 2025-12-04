import { isNullish } from '../util';
import { CodecDefinition } from './common';

/**
 * Parameter string:
 *   1. https://www.webmproject.org/vp9/mp4/#syntax_1
 *   2. https://www.webmproject.org/vp9/mp4/#codecs-parameter-string
 */
export type CodecVp09Parameters = {
  profile: number | string; // 0..3
  level: number | string; // 10..63
  bitDepth: number | string; // 8, 10, 12
  chromaSubsampling?: number | string; // 0..3
  videoFullRangeFlag?: 0 | 1 | string;
  colourPrimaries?: number | string; // 0..255
  transferCharacteristics?: number | string; // 0..255
  matrixCoefficients?: number | string; // 0..255
};

export function vp09ToParamString(codec: 'vp09', params: CodecVp09Parameters) {
  let acc = '' + codec;
  acc += `.${params.profile.toString().padStart(2, '0')}`;
  acc += `.${params.level.toString().padStart(2, '0')}`;
  acc += `.${params.bitDepth.toString().padStart(2, '0')}`;
  // If none of the optional parameters are set, return early
  if (
    isNullish(params.chromaSubsampling) &&
    isNullish(params.videoFullRangeFlag) &&
    isNullish(params.colourPrimaries) &&
    isNullish(params.transferCharacteristics) &&
    isNullish(params.matrixCoefficients)
  ) {
    return acc;
  }

  // Defaults from https://www.webmproject.org/vp9/mp4/#optional-fields
  acc += `.${params.chromaSubsampling?.toString().padStart(2, '0') || '00'}`;
  acc += `.${params.videoFullRangeFlag?.toString() || '0'}`;
  acc += `.${params.colourPrimaries?.toString().padStart(2, '0') || '00'}`;
  acc += `.${
    params.transferCharacteristics?.toString().padStart(2, '0') || '00'
  }`;
  acc += `.${params.matrixCoefficients?.toString().padStart(2, '0') || '00'}`;

  return acc;
}

/**
 * Overview: https://www.webmproject.org/vp9/mp4/#codecs-parameter-string
 *
 * @see {@link CodecVp09Parameters}
 */
export type CodecVp09 = CodecDefinition<CodecVp09Parameters, 'vp09'>;

/**
 * @see {@link CodecVp09}
 */
export const CODEC_VP09: CodecVp09 = {
  codec: 'vp09',
  displayName: 'VP9',
  toParamString: vp09ToParamString,
};
