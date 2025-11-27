import { Injectable } from '@angular/core';
import { defer, retry, tap } from 'rxjs';
import { WebSocketSubject } from 'rxjs/webSocket';
import { WsHandlers, connect_ws } from 'dev-disp-ws-js';

@Injectable({ providedIn: 'root' })
export class DevDispService {
  connect(address: string) {
    const handlers: WsHandlers = {
      onCore: () => {},
      onRequestDeviceInfo: () => {},
      onRequestPreInit: () => {},
      onRequestProtocolInit: () => {},
    };

    const cancelConnection = connect_ws('127.0.0.1:56789', handlers);

    cancelConnection();

    return new DevDispConnection(address);
  }
}

export class DevDispConnection {
  private readonly connection$: WebSocketSubject<ArrayBuffer>;

  public readonly anyData$ = defer(() => {
    console.log(`Subscribing to dev-disp subject...`);
    return this.connection$;
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
