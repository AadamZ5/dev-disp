import { DestroyRef, inject, Injectable } from '@angular/core';
import {
  JsDisplayParameters,
  WsDispatchers,
  WsHandlers,
  connectDevDispServer,
} from 'dev-disp-ws-js';
import {
  BehaviorSubject,
  Observable,
  ReplaySubject,
  share,
  Subscription,
} from 'rxjs';

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

export class DevDispConnection {
  private readonly dispatchers: WsDispatchers;

  private readonly _screenData$ = new ReplaySubject<ArrayBuffer | undefined>(1);
  public readonly screenData$ = this._screenData$.asObservable();

  private readonly _connected$ = new BehaviorSubject<boolean>(false);
  public readonly connected$ = this._connected$.asObservable();

  private readonly _disconnect$ = new ReplaySubject<DevDispEventDisconnect>(1);
  public readonly disconnect$ = this._disconnect$.asObservable();

  private intentionalDisconnect = false;

  constructor(public readonly address: string, canvas?: OffscreenCanvas) {
    const context2d = canvas?.getContext('2d');

    const decode = new VideoDecoder({
      output: (frame) => {
        console.log('Decoded frame:', frame);
        context2d?.drawImage(
          frame,
          0,
          0,
          frame.displayWidth,
          frame.displayHeight
        );
        frame.close();
      },
      error: (e) => {
        console.error('VideoDecoder error:', e);
      },
    });

    const handlers: WsHandlers = {
      onPreInit: () => {
        console.log('Dev-disp pre-init requested');
      },
      onProtocolInit: () => {
        console.log('Dev-disp protocol init requested');
      },
      onConnect: (e) => {
        console.log('Dev-disp connected', e);
        this._connected$.next(true);
      },
      onDisconnect: (e) => {
        console.log('Dev-disp disconnected', e);
        // TODO: Check reason code here to see if it was an
        // explicit disconnect from the server!
        if (!this.intentionalDisconnect) {
          console.warn('Dev-disp unintentional disconnect!');
        }
        this._disconnect$.next({
          intentional: this.intentionalDisconnect,
          wsReason: e.error,
        });
        this._connected$.next(false);

        this._complete();
      },
      handleRequestDeviceInfo: (e) => {
        console.log('Dev-disp device info requested', e);
        return {
          name: 'Web Testpage Display',
          resolution: [800, 600],
        };
      },
      handleScreenData: (e) => {
        console.log('Dev-disp screen data received', e);

        if (!e?.data) {
          return;
        }
        const chunk = new EncodedVideoChunk({
          data: new Uint8Array(e?.data!),
          timestamp: 0,
          type: 'key',
        });
        decode.decode(chunk);

        this._screenData$.next(e?.data);
      },
      handleRequestDisplayParameters: (e) => {
        console.log('Dev-disp display parameters requested', e);
        return {
          name: 'Web Testpage Display',
          resolution: [800, 600],
        };
      },
      handleRequestPreferredEncoding: async (configs) => {
        console.log('Dev-disp preferred encodings requested', configs);

        const compatibleConfigResults = await Promise.allSettled(
          configs.map(async (cfg) => {
            return VideoDecoder.isConfigSupported({
              codec: cfg.encoderFamily,
              codedHeight: cfg.encodedResolution[0],
              codedWidth: cfg.encodedResolution[1],
            }).then((supportResult) => {
              console.log(
                `Config support result for ${cfg.encoderFamily} (${cfg.encoderName}):`,
                supportResult.supported
              );
              return {
                supportResult,
                config: cfg,
              };
            });
          })
        );

        return compatibleConfigResults
          .filter((result) => {
            return (
              result.status === 'fulfilled' &&
              result.value.supportResult.supported
            );
          })
          .map((result) => {
            const fulfilled = result as PromiseFulfilledResult<{
              supportResult: VideoDecoderSupport;
              config: (typeof configs)[number];
            }>;
            return fulfilled.value.config;
          });
      },
      handleSetEncoding: (encodingConfig) => {
        console.log('Dev-disp set encoding requested', encodingConfig);

        decode.configure({
          codec: encodingConfig.encoderFamily,
          codedHeight: encodingConfig.encodedResolution[0],
          codedWidth: encodingConfig.encodedResolution[1],
        });
      },
    };

    this.dispatchers = connectDevDispServer(
      address,
      handlers,
      canvas ?? new OffscreenCanvas(1, 1)
    );
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
    this._screenData$.complete();
    this._disconnect$.complete();
    this._connected$.complete();
  }
}

export function fromDevDispConnection(
  factory: () => DevDispConnection
): Observable<ArrayBuffer | undefined> {
  return new Observable<ArrayBuffer | undefined>((subscriber) => {
    const devDispConnection = factory();

    const disconnectSub = devDispConnection.disconnect$.subscribe({
      next: (e) => {
        if (!e.intentional) {
          subscriber.error(
            new Error('Dev-disp connection disconnected unexpectedly')
          );
        }
      },
    });

    const screenDataSub = devDispConnection.screenData$.subscribe({
      next: (data) => {
        subscriber.next(data);
      },
      error: (err) => {
        subscriber.error(err);
      },
      complete: () => {
        subscriber.complete();
      },
    });

    const allSub = new Subscription(() => {
      devDispConnection.disconnect();
    });
    allSub.add(disconnectSub);
    allSub.add(screenDataSub);

    return allSub;
  }).pipe(share());
}
