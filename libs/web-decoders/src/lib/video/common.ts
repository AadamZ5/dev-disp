export type CodecParameterStringFn<
  C extends string = string,
  T extends Record<string, string | number> | null = Record<
    string,
    string | number
  > | null
> = (codec: C, params: T) => string;

export type CodecDefinition<
  T extends Record<string, string | number> | null,
  C extends string = string
> = Readonly<{
  codec: C;
  displayName?: string;
  toParamString: CodecParameterStringFn<C, T>;
}>;
