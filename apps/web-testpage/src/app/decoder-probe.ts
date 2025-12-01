export type ProbeEncoder = {
  codec: string;
  hardwareAcceleration: HardwareAcceleration;
};

// Pain in the ass. Refer to:
// https://www.w3.org/TR/webcodecs-codec-registry/#video-codec-registry

export const PROBE_ENCODERS: ProbeEncoder[] = [
  {
    // Overview: https://www.w3.org/TR/webcodecs-av1-codec-registration/
    // Parameter string:
    //   - https://aomediacodec.github.io/av1-isobmff/#codecsparam
    codec: 'av01',
    hardwareAcceleration: 'prefer-hardware',
  },
  {
    // Overview: https://www.w3.org/TR/webcodecs-av1-codec-registration/
    // Parameter string:
    //   - https://www.rfc-editor.org/rfc/rfc6381#section-3.4
    //   - Section 5.4.1 of https://www.iso.org/standard/89118.html
    codec: 'avc1',
    hardwareAcceleration: 'prefer-hardware',
  },
  {
    codec: 'h265',
    hardwareAcceleration: 'prefer-hardware',
  },
];
