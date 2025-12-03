import { CODEC_AV1 } from './av1';
import { CODEC_VP09 } from './vp09';
import { CODEC_VP8 } from './vp8';

// import { CODEC_HEV1, CODEC_HVC1 } from './hevc';
// import { CODEC_AVC1, CODEC_AVC3 } from './avc';

export * from './av1';
export * from './avc';
export * from './hevc';
export * from './vp8';
export * from './vp09';

export const VIDEO_CODEC_DEFINITIONS = [
  CODEC_AV1,
  CODEC_VP8,
  CODEC_VP09,
  // These aren't supported yet, because we don't know the parameters!
  // CODEC_AVC1,
  // CODEC_AVC3,
  // CODEC_HEV1,
  // CODEC_HVC1,
] as const;

export type VideoCodecDefinition = (typeof VIDEO_CODEC_DEFINITIONS)[number];

// TODO: Better const-literal type
export const VIDEO_CODEC_IDS = VIDEO_CODEC_DEFINITIONS.map(
  (def) => def.codec
) as ReadonlyArray<VideoCodecDefinition['codec']>;

export type VideoCodecId = (typeof VIDEO_CODEC_IDS)[number];

// TODO: Better const-literal type
export const VIDEO_CODEC_NAMES = VIDEO_CODEC_DEFINITIONS.map((def) =>
  def.displayName ? def.displayName : def.codec
) as ReadonlyArray<string>;
