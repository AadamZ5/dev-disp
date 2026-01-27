import { DestroyRef, inject, Injectable } from '@angular/core';
import {
  DevDispEvent,
  JsDisplayParameters,
  JsEncoderPossibleConfiguration,
  WsDispatchers,
  WsHandlers,
  connectDevDispServer,
} from 'dev-disp-ws-js';
import { CodecParameterStringFn } from 'libs/web-decoders/src/lib/video/common';
import {
  BehaviorSubject,
  Observable,
  ReplaySubject,
  share,
  Subject,
  Subscription,
} from 'rxjs';
import { SearchCodecResult, searchSupportedVideoDecoders } from 'web-decoders';

@Injectable({ providedIn: 'root' })
export class DevDispService {
  private readonly destroyRef = inject(DestroyRef);

  connect(address: string, canvas?: OffscreenCanvas): DevDispConnection {
    const connection = new DevDispConnection(address, canvas);
    this.destroyRef.onDestroy(() => {
      connection.disconnect();
    });
    return connection;
  }
}

export type DevDispEventDisconnect = {
  intentional: boolean;
  wsReason?: number;
};

export type DevDispEncoding = {
  encoderName: string;
  encoderFamily: string;
  encodedResolution: [number, number];
  parameters: Map<string, string>;
  webCodecString: string;
};

export class DevDispConnection {
  private readonly dispatchers: WsDispatchers;

  private readonly _decodedFrame$ = new Subject<void>();
  /** Just a notification that a frame has been decoded */
  public readonly decodedFrame$ = this._decodedFrame$.asObservable();

  private readonly _connected$ = new BehaviorSubject<boolean>(false);
  /** Connected state */
  public readonly connected$ = this._connected$.asObservable();

  private readonly _disconnect$ = new ReplaySubject<DevDispEventDisconnect>(1);
  /** Emits on disconnection */
  public readonly disconnect$ = this._disconnect$.asObservable();

  private readonly _configuredEncoding$ = new ReplaySubject<DevDispEncoding>(1);
  /** Emits after the encoding has been negotiated and agreed on */
  public readonly configuredEncoding$ =
    this._configuredEncoding$.asObservable();

  private readonly drawer?: ((frame: VideoFrame) => void) | null;
  private readonly decoder = new VideoDecoder({
    output: this.onDecode.bind(this),
    error: this.onDecodeError.bind(this),
  });

  private supportedDecoderConfigurations: SearchCodecResult[] = [];
  private intentionalDisconnect = false;

  constructor(
    public readonly address: string,
    private readonly canvas: OffscreenCanvas = new OffscreenCanvas(800, 640),
  ) {
    console.log(
      `Got canvas with size ${this.canvas.width}x${this.canvas.height}`,
    );

    this.drawer = this._getDrawer(this.canvas);

    const handlers: WsHandlers = {
      onConnect: this.onConnect.bind(this),
      onDisconnect: this.onDisconnect.bind(this),
      handleRequestDeviceInfo: this.onRequestDeviceInfo.bind(this),
      handleScreenData: this.onScreenData.bind(this),
      handleRequestDisplayParameters: this.onRequestDeviceInfo.bind(this),
      handleRequestPreferredEncoding:
        this.handleRequestPreferredEncodings.bind(this),
      handleSetEncoding: this.handleSetEncoding.bind(this),
    };

    this.dispatchers = connectDevDispServer(address, handlers);
  }

  private _getDrawer(canvas: OffscreenCanvas) {
    const contextWebGl2 = canvas.getContext('webgl2');
    if (contextWebGl2) {
      return contextWebgl2Drawer(contextWebGl2);
    }

    const context2d = canvas.getContext('2d');
    if (context2d) {
      return context2dDrawer(context2d);
    }

    return null;
  }

  disconnect() {
    this.intentionalDisconnect = true;
    const result = this.dispatchers.closeConnection();
    this._complete();
    return result;
  }

  updateDisplayParameters(params: JsDisplayParameters) {
    return this.dispatchers.updateDisplayParameters(params);
  }

  private _complete() {
    this._decodedFrame$.complete();
    this._disconnect$.complete();
    this._connected$.complete();
    this._configuredEncoding$.complete();
  }

  private onDecode(frame: VideoFrame) {
    this.drawer?.(frame);
    frame.close();
    this._decodedFrame$.next();
  }

  private onDecodeError(e: DOMException) {
    console.error('VideoDecoder error:', e);
    if (this.decoder.state === 'closed') {
      console.warn('Decoder is closed, cannot recover from error');
      // Do a non-intentional disconnect
      this.intentionalDisconnect = false;
      this.dispatchers.closeConnection();
    }
  }

  private onConnect(e: unknown) {
    console.log('Dev-disp connected', e);
    this._connected$.next(true);
  }

  private onDisconnect(e: DevDispEvent) {
    if (!this.intentionalDisconnect) {
      console.warn('Dev-disp unintentional disconnect!', e);
    } else {
      console.log('Dev-disp intentional disconnect', e);
    }

    this._disconnect$.next({
      intentional: this.intentionalDisconnect,
      wsReason: e.error,
    });
    this._connected$.next(false);
    this._complete();
  }

  private onRequestDeviceInfo(e: DevDispEvent): JsDisplayParameters {
    console.log('Dev-disp device info requested', e);
    return {
      name: 'Web Testpage Display',
      resolution: [this.canvas.width, this.canvas.height],
    };
  }

  private onScreenData(e: DevDispEvent | null): void {
    if (!e?.data) {
      return;
    }

    // If we have a shared buffer and the data is a number, use that
    // as the byte-length to read from the shared buffer
    let data: Uint8Array;
    if (this.dispatchers.screenData && typeof e.data === 'number') {
      const sharedBuffer = this.dispatchers.screenData;
      data = new Uint8Array(sharedBuffer, 0, e.data as number);
    } else if (e.data instanceof Uint8Array) {
      data = e.data;
    } else if (e.data instanceof ArrayBuffer) {
      // This shouldn't happen
      data = new Uint8Array(e.data);
    } else {
      // Unknown data type
      return;
    }

    const chunk = new EncodedVideoChunk({
      data,
      timestamp: 0,
      // I don't know what this is doing
      type: 'key',
    });

    this.decoder.decode(chunk);
  }

  private async handleRequestPreferredEncodings(
    configs: JsEncoderPossibleConfiguration[],
  ) {
    console.log('Dev-disp preferred encodings requested', configs);

    const compatibleConfigResults = await Promise.allSettled(
      configs.map(async (cfg) => {
        const parameters = Object.fromEntries(cfg.parameters);
        const supportedDecoders = await searchSupportedVideoDecoders(
          cfg.encoderFamily,
          parameters,
          cfg.encodedResolution[0],
          cfg.encodedResolution[1],
        );
        return {
          supportedDecoders,
          sentConfig: cfg,
        };
      }),
    );

    const flattenedResults = compatibleConfigResults
      .filter(
        (
          result,
        ): result is PromiseFulfilledResult<{
          supportedDecoders: SearchCodecResult[];
          sentConfig: JsEncoderPossibleConfiguration;
        }> => {
          if (result.status === 'rejected') {
            console.error(`Decoding check failed`, result.reason);
          } else if (
            result.status === 'fulfilled' &&
            result.value.supportedDecoders.length <= 0
          ) {
            console.log(
              `Decoding not supported for "${result.value.sentConfig.encoderFamily}"`,
            );
          }

          return (
            result.status === 'fulfilled' &&
            result.value.supportedDecoders.length > 0
          );
        },
      )
      .flatMap((result) => {
        return result.value.supportedDecoders.map((supportedDecoder) => {
          return {
            supportedDecoder,
            sentConfig: result.value.sentConfig,
          };
        });
      });

    this.supportedDecoderConfigurations = flattenedResults.map(
      (r) => r.supportedDecoder,
    );

    const possibleConfigurations = flattenedResults.map((configuration) => {
      const supportRes = configuration.supportedDecoder.decoderConfig
        ? ([
            configuration.supportedDecoder.decoderConfig.codedWidth ??
              configuration.sentConfig.encodedResolution[0],
            configuration.supportedDecoder.decoderConfig.codedHeight ??
              configuration.sentConfig.encodedResolution[1],
          ] as const)
        : configuration.sentConfig.encodedResolution;

      return {
        encoderName: configuration.sentConfig.encoderName,
        encoderFamily: configuration.supportedDecoder.definition.codec,
        encodedResolution: supportRes as [number, number],
        parameters: configuration.sentConfig.parameters,
      } satisfies JsEncoderPossibleConfiguration;
    });

    possibleConfigurations.forEach((result) => {
      console.log(
        `Supported encoding found for ${result.encoderFamily}`,
        result,
      );
    });

    return possibleConfigurations;
  }

  private handleSetEncoding(encodingConfig: JsEncoderPossibleConfiguration) {
    console.log('Dev-disp set encoding requested', encodingConfig);

    const correspondingDecoder = this.supportedDecoderConfigurations.find(
      (decodingConfig) =>
        decodingConfig.definition.codec === encodingConfig.encoderFamily,
    );
    if (!correspondingDecoder) {
      console.error(
        `No supported decoder found for requested encoding:`,
        encodingConfig,
      );
      return;
    }

    this.canvas.width = encodingConfig.encodedResolution[0];
    this.canvas.height = encodingConfig.encodedResolution[1];

    const webCodecString = (
      correspondingDecoder.definition.toParamString as CodecParameterStringFn
    )(
      correspondingDecoder.definition.codec,
      Object.fromEntries(encodingConfig.parameters),
    );

    this.decoder.configure({
      codec: webCodecString,
      codedWidth: encodingConfig.encodedResolution[0],
      codedHeight: encodingConfig.encodedResolution[1],
    });

    this._configuredEncoding$.next({
      encoderName: encodingConfig.encoderName,
      encoderFamily: encodingConfig.encoderFamily,
      encodedResolution: encodingConfig.encodedResolution,
      parameters: encodingConfig.parameters,
      webCodecString,
    });
  }
}

export function ofDevDispConnection(
  factory: () => DevDispConnection,
): Observable<DevDispConnection> {
  return new Observable<DevDispConnection>((subscriber) => {
    const devDispConnection = factory();

    const disconnectSub = devDispConnection.disconnect$.subscribe({
      next: (e) => {
        if (!e.intentional) {
          subscriber.error(
            new Error('Dev-disp connection disconnected unexpectedly'),
          );
        }
        subscriber.complete();
      },
      error: (err) => {
        subscriber.error(err);
      },
    });

    let syncRan = false;
    const connectionSub = devDispConnection.connected$.subscribe({
      next: (connected) => {
        if (connected) {
          subscriber.next(devDispConnection);
          syncRan = true;
          if (connectionSub) {
            connectionSub.unsubscribe();
          }
        }
      },
    });
    if (syncRan) {
      connectionSub.unsubscribe();
    }

    const allSub = new Subscription();
    allSub.add(disconnectSub);
    allSub.add(connectionSub);
    allSub.add(() => {
      devDispConnection.disconnect();
    });

    return allSub;
  }).pipe(share());
}

export function context2dDrawer(
  context: OffscreenCanvasRenderingContext2D,
): (frame: VideoFrame) => void {
  return (frame: VideoFrame) => {
    context.canvas.width = frame.displayWidth;
    context.canvas.height = frame.displayHeight;
    context.drawImage(frame, 0, 0, frame.displayWidth, frame.displayHeight);
    frame.close();
  };
}

export function contextWebgl2Drawer(
  gl: WebGL2RenderingContext,
): (frame: VideoFrame) => void {
  const vertexShader = `#version 300 es
  in vec2 a_position;
  in vec2 a_texCoord;
  out vec2 v_texCoord;
  void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_texCoord = a_texCoord;
  }
  `;

  const fragmentShader = `#version 300 es
  precision mediump float;
  in vec2 v_texCoord;
  uniform sampler2D u_texture;
  out vec4 outColor;
  void main() {
    outColor = texture(u_texture, v_texCoord);
  }
  `;

  const vs = gl.createShader(gl.VERTEX_SHADER);
  if (!vs) {
    throw new Error('Failed to create vertex shader');
  }
  gl.shaderSource(vs, vertexShader);
  gl.compileShader(vs);
  if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
    console.error('Vertex shader error:', gl.getShaderInfoLog(vs));
  }

  const fs = gl.createShader(gl.FRAGMENT_SHADER);
  if (!fs) {
    throw new Error('Failed to create fragment shader');
  }
  gl.shaderSource(fs, fragmentShader);
  gl.compileShader(fs);
  if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
    console.error('Fragment shader error:', gl.getShaderInfoLog(fs));
  }

  const program = gl.createProgram();
  if (!program) {
    throw new Error('Failed to create program');
  }
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error('Program link error:', gl.getProgramInfoLog(program));
    throw new Error('Failed to link program');
  }

  const vertices = new Float32Array([
    -1, -1, 0, 1, 1, -1, 1, 1, -1, 1, 0, 0, 1, 1, 1, 0,
  ]);

  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);

  const vertexBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);

  const aPositionLoc = gl.getAttribLocation(program, 'a_position');
  gl.enableVertexAttribArray(aPositionLoc);
  gl.vertexAttribPointer(aPositionLoc, 2, gl.FLOAT, false, 16, 0);

  const aTexCoordLoc = gl.getAttribLocation(program, 'a_texCoord');
  gl.enableVertexAttribArray(aTexCoordLoc);
  gl.vertexAttribPointer(aTexCoordLoc, 2, gl.FLOAT, false, 16, 8);

  const texture = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);

  return (frame: VideoFrame) => {
    gl.viewport(0, 0, frame.codedWidth, frame.codedHeight);

    gl.useProgram(program);
    gl.bindVertexArray(vao);

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, frame);

    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    frame.close();
  };
}
