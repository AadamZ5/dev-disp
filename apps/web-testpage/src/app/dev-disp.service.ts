import { Injectable } from '@angular/core';
import { defer, EMPTY, retry, tap } from 'rxjs';
import { WebSocketSubject } from 'rxjs/webSocket';
import { WsHandlers, connect_ws } from 'dev-disp-ws-js';

export class InnerDevDispConnection {
  constructor(public readonly address: string) {}
}

@Injectable({ providedIn: 'root' })
export class DevDispService {
  connect(address: string) {
    const handlers: WsHandlers = {
      onCore: (e) => {
        console.log('Dev-disp core message received', e, e.data);
      },
      onPreInit: () => {
        console.log('Dev-disp pre-init requested');
      },
      onProtocolInit: () => {
        console.log('Dev-disp protocol init requested');
      },
      onConnect: (e) => {
        console.log('Dev-disp connected', e);
      },
      onDisconnect: (e) => {
        console.log('Dev-disp disconnected', e);
      },
      handleRequestDeviceInfo: (e) => {
        console.log('Dev-disp device info requested', e);
        return {};
      },
      handleScreenData: (e) => {
        console.log('Dev-disp screen data received', e);
      },
      handleRequestDisplayParameters: (e) => {
        console.log('Dev-disp display parameters requested', e);
        return {
          name: 'Web Testpage Display',
          resolution: [800, 600],
        };
      },
    };

    const cancelConnection = connect_ws('127.0.0.1:56789', handlers);

    return new DevDispConnection(address);
  }
}

export class DevDispConnection {
  private readonly connection$: WebSocketSubject<ArrayBuffer>;

  public readonly anyData$ = defer(() => {
    return EMPTY;
  }).pipe(
    tap({ error: (e) => console.log(`Error from dev-disp subject:`, e) }),
    retry({ delay: 5000 })
  );

  constructor(public readonly address: string) {
    this.connection$ = new WebSocketSubject<ArrayBuffer>({
      url: this.address,
      openObserver: {
        next: (event) => {
          console.log(`Connected to dev-disp`, event);
        },
      },
      closeObserver: {
        next: (event) => {
          console.log(`Disconnected from dev-disp`, event);
        },
      },
      binaryType: 'arraybuffer',
      deserializer: (e) => e.data,
      serializer: (value) => value,
    });
  }

  send(data: ArrayBuffer) {
    this.connection$.next(data);
  }

  destroy() {
    this.connection$.complete();
  }
}
