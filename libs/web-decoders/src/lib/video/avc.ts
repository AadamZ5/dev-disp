import { CodecDefinition } from './common';

/**
 * Parameter string:
 *  1. Section 5.4.1 of https://www.iso.org/standard/89118.html
 *  2. https://www.rfc-editor.org/rfc/rfc6381#section-3.4
 */
export type CodecAvcParameters = {
  // TODO: Get a document that explains the parameters!
};

/**
 * Overview: https://www.w3.org/TR/webcodecs-av1-codec-registration/
 *
 * @see {@link CodecAvcParameters}
 */
export type CodecAvc1 = CodecDefinition<CodecAvcParameters, 'avc1'>;
export type CodecAvc3 = CodecDefinition<CodecAvcParameters, 'avc3'>;

function avcParamString(codec: 'avc1' | 'avc3', params: CodecAvcParameters) {
  let acc = '' + codec;
  // TODO: Implement the toParamString function for AVC
  return acc;
}

export const CODEC_AVC1: CodecAvc1 = {
  codec: 'avc1',
  toParamString: avcParamString,
};

export const CODEC_AVC3: CodecAvc3 = {
  codec: 'avc3',
  toParamString: avcParamString,
};
