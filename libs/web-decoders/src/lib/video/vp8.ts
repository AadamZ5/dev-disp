import { CodecDefinition } from './common';

export type CodecVp8Parameters = null;

export type CodecVp8Definition = CodecDefinition<CodecVp8Parameters, 'vp8'>;

export const CODEC_VP8: CodecVp8Definition = {
  codec: 'vp8',
  displayName: 'VP8',
  toParamString: (codec: 'vp8') => {
    // VP8 has no parameters
    return codec;
  },
};
