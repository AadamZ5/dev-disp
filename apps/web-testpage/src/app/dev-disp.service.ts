import { Injectable } from '@angular/core';
import { defer, retry, tap } from 'rxjs';
import { WebSocketSubject } from 'rxjs/webSocket';

@Injectable({ providedIn: 'root' })
export class DevDispService {
  connect(address: string) {}
}

export class DevDispConnection {
  private readonly connection$: WebSocketSubject<unknown>;

  private readonly anyData$ = defer(() => {
    console.log(`Subscribing to dev-disp subject...`);
    return this.connection$;
  }).pipe(
    tap({ error: (e) => console.log(`Error from dev-disp subject:`, e) }),
    retry({ delay: 5000 })
  );

  constructor(public readonly address: string) {
    this.connection$ = new WebSocketSubject<unknown>({
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
    });
  }

  destroy() {
    this.connection$.complete();
  }
}
