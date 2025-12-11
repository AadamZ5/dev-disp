import { isNullish } from '../util';
import { CodecDefinition } from './common';

/**
 * Parameter string:
 *  1. Section 5.4.1 of https://www.iso.org/standard/89118.html
 *  2. https://www.rfc-editor.org/rfc/rfc6381#section-3.4
 */
export type CodecAvcParameters = {
  profile: number | string;
  constraintFlags: number | string;
  level: number | string;
};

/**
 * Overview: https://www.w3.org/TR/webcodecs-av1-codec-registration/
 *
 * @see {@link CodecAvcParameters}
 */
export type CodecAvc1 = CodecDefinition<CodecAvcParameters, 'avc1'>;
/**
 * Overview: https://www.w3.org/TR/webcodecs-av1-codec-registration/
 *
 * @see {@link CodecAvcParameters}
 */
export type CodecAvc3 = CodecDefinition<CodecAvcParameters, 'avc3'>;

function avcParamString(codec: 'avc1' | 'avc3', params: CodecAvcParameters) {
  let acc = '' + codec + '.';
  if (!isNullish(params.profile)) {
    acc += Number(params.profile).toString(16).padStart(2, '0');
  } else {
    acc += '00';
  }
  if (!isNullish(params.constraintFlags)) {
    acc += Number(params.constraintFlags).toString(16).padStart(2, '0');
  } else {
    acc += '00';
  }
  if (!isNullish(params.level)) {
    acc += Number(params.level).toString(16).padStart(2, '0');
  } else {
    acc += '00';
  }
  return acc;
}

/**
 * @see {@link CodecAvc1}
 */
export const CODEC_AVC1: CodecAvc1 = {
  codec: 'avc1',
  toParamString: avcParamString,
};

/**
 * @see {@link CodecAvc3}
 */
export const CODEC_AVC3: CodecAvc3 = {
  codec: 'avc3',
  displayName: 'H.264',
  toParamString: avcParamString,
};
