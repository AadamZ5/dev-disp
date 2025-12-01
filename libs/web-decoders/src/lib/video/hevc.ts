import { CodecDefinition } from './common';

/**
 * Parameter string:
 *   1. Section E.3 of https://www.iso.org/standard/79312.html
 */
export type CodecHevcParameters = {
  /**
   *  - 1 = Main profile
   *  - 2 = Main 10 profile
   *  - Others unknown
   */
  profile: number;
  /** Bitmask indicating other profiles compatible */
  compatibility: number;
  /**
   * 'L' = Main Tier
   * 'H' = High Tier
   */
  tier: 'L' | 'H';
  /**
   * Level indication as per Table E.3 of https://www.iso.org/standard/79312.html
   *
   * Multiply desired level by 30 to get the level indication.
   * Ex:
   * Level 3.1 is specified as 93.
   * Level 4.0 is specified as 120.
   * Level 4.1 is specified as 123.
   * Level 5.1 is specified as 153.
   */
  level: number;
  /**
   * BE Bitmask of constraint bytes
   *
   * Unknown representation of logical parameters
   *  - `frame_only`
   *  - `non_packed`
   *  - `interlaced_source`
   *  - Others unknown
   */
  constraintBytes: number;
};

export type CodecHevc = CodecDefinition<CodecHevcParameters, 'hvc1' | 'hev1'>;

function hevcParamString(codec: 'hvc1' | 'hev1', params: CodecHevcParameters) {
  let acc = '' + codec;
  acc += `.${params.profile.toString()}`;
  acc += `.${params.compatibility.toString()}`;
  acc += `.${params.tier}${params.level.toString().padStart(2, '0')}`;
  acc += `.${params.constraintBytes.toString(16).padStart(2, '0')}`;
  return acc;
}

export const CODEC_HVC1: CodecHevc = {
  codec: 'hvc1',
  toParamString: hevcParamString,
};

export const CODEC_HEV1: CodecHevc = {
  codec: 'hev1',
  toParamString: hevcParamString,
};
