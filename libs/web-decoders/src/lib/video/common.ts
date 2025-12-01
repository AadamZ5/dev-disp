export type CodecDefinition<
  T extends Record<string, any> | null,
  C extends string = string
> = {
  codec: C;
  toParamString: (codec: C, params: T) => string;
};
