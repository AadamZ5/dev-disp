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
  public readonly decodedFrame$ = this._decodedFrame$.asObservable();

  private readonly _connected$ = new BehaviorSubject<boolean>(false);
  public readonly connected$ = this._connected$.asObservable();

  private readonly _disconnect$ = new ReplaySubject<DevDispEventDisconnect>(1);
  public readonly disconnect$ = this._disconnect$.asObservable();

  private readonly _configuredEncoding$ = new ReplaySubject<DevDispEncoding>(1);
  public readonly configuredEncoding$ =
    this._configuredEncoding$.asObservable();

  private readonly canvasContext?: OffscreenCanvasRenderingContext2D | null;
  private readonly decoder = new VideoDecoder({
    output: this.onDecode.bind(this),
    error: this.onDecodeError.bind(this),
  });

  private supportedDecoderConfigurations: SearchCodecResult[] = [];
  private intentionalDisconnect = false;

  constructor(
    public readonly address: string,
    private readonly canvas: OffscreenCanvas = new OffscreenCanvas(2, 2),
  ) {
    this.canvasContext = canvas?.getContext('2d');

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
    this.canvasContext?.drawImage(
      frame,
      0,
      0,
      frame.displayWidth,
      frame.displayHeight,
    );
    frame.close();
    this._decodedFrame$.next();
  }

  private onDecodeError(e: DOMException) {
    console.error('VideoDecoder error:', e);
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

    this.supportedDecoderConfigurations = [];

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
        this.supportedDecoderConfigurations.push(
          ...result.value.supportedDecoders,
        );

        return result.value.supportedDecoders.map((supportedDecoder) => {
          return {
            supportedDecoder,
            sentConfig: result.value.sentConfig,
          };
        });
      })
      .map((configuration) => {
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

    flattenedResults.forEach((result) => {
      console.log(
        `Supported encoding found for ${result.encoderFamily}`,
        result,
      );
    });

    return flattenedResults;
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
