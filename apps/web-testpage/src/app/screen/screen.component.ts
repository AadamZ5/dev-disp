import {
  Component,
  effect,
  ElementRef,
  inject,
  viewChild,
} from '@angular/core';
import {
  distinctUntilChanged,
  endWith,
  filter,
  fromEvent,
  map,
  retry,
  scan,
  share,
  shareReplay,
  startWith,
  Subscription,
  switchMap,
  tap,
  throttleTime,
} from 'rxjs';
import { DevDispService, ofDevDispConnection } from '../dev-disp.service';
import { toObservable, toSignal } from '@angular/core/rxjs-interop';
import { slidingWindow } from 'web-util';

@Component({
  selector: 'app-screen',
  imports: [],
  templateUrl: './screen.component.html',
  styleUrl: './screen.component.scss',
})
export class ScreenComponent {
  private readonly devDispService = inject(DevDispService);
  private readonly wakeLocker = new WakeLocker();
  readonly canvas = viewChild<ElementRef<HTMLCanvasElement>>('screen');

  private readonly canvas$ = toObservable(this.canvas);

  private readonly resolution$ = this.canvas$.pipe(
    filter((canvas): canvas is ElementRef<HTMLCanvasElement> => !!canvas),
    // TODO: Use ResizeObserver to detect canvas size changes
    map((canvas) => {
      return [
        canvas.nativeElement.clientWidth * window.devicePixelRatio,
        canvas.nativeElement.clientHeight * window.devicePixelRatio,
      ] as const;
    }),
    shareReplay(1),
  );

  readonly offscreenCanvas$ = this.canvas$.pipe(
    distinctUntilChanged(),
    map((canvas) => {
      const offscreen = canvas?.nativeElement.transferControlToOffscreen();
      if (offscreen && canvas) {
        offscreen.width =
          canvas.nativeElement.clientWidth * window.devicePixelRatio;
        offscreen.height =
          canvas.nativeElement.clientHeight * window.devicePixelRatio;
      }
      return offscreen;
    }),
    distinctUntilChanged(),
    shareReplay(1),
  );

  readonly client$ = this.offscreenCanvas$.pipe(
    switchMap((offscreenCanvas) =>
      ofDevDispConnection(() =>
        this.devDispService.connect(
          `${window.location.hostname}:56789`,
          offscreenCanvas,
        ),
      ),
    ),

    tap({
      error: (e) => {
        console.error('Dev-disp connection error', e);
      },
    }),
    retry({ delay: 5000 }),
    share(),
  );

  readonly dataEpoch = toSignal(
    this.client$.pipe(
      switchMap((client) =>
        client.decodedFrame$.pipe(
          scan((acc) => {
            return acc + 1;
          }, 0),
          endWith(-1),
        ),
      ),
    ),
    { initialValue: -1 },
  );

  readonly fps = toSignal(
    this.client$.pipe(
      switchMap((client) =>
        client.decodedFrame$.pipe(
          map(() => performance.now()),
          slidingWindow(30),
          throttleTime(50, undefined, { leading: true, trailing: true }),
          map((times) => {
            if (times.length < 2) {
              return 0;
            }
            const duration = times[0] - times[times.length - 1];
            return ((times.length - 1) * 1000) / duration;
          }),
          endWith(0),
        ),
      ),
    ),
    { initialValue: 0 },
  );

  readonly configuredEncoding = toSignal(
    this.client$.pipe(
      switchMap((conn) => conn.configuredEncoding$.pipe(endWith(null))),
    ),
    { initialValue: null },
  );

  readonly connected = toSignal(
    this.client$.pipe(
      switchMap((client) =>
        client.connected$.pipe(distinctUntilChanged(), endWith(false)),
      ),
    ),
    { initialValue: false },
  );

  constructor() {
    this._effectConnectedLockScreen();
  }

  private _effectConnectedLockScreen() {
    const documentVisible = toSignal(
      fromEvent(document, 'visibilitychange').pipe(
        map(() => document.visibilityState === 'visible'),
        startWith(document.visibilityState === 'visible'),
        distinctUntilChanged(),
      ),
      { initialValue: document.visibilityState === 'visible' },
    );

    effect(() => {
      if (!documentVisible()) {
        return;
      }

      if (this.connected()) {
        this.wakeLocker
          .lock()
          .then(() => {
            console.log('Wake lock acquired');
          })
          .catch((e) => {
            console.error('Failed to acquire wake lock', e);
          });
      } else {
        this.wakeLocker.unlock();
      }
    });
  }
}

export class WakeLocker {
  private wantLocked = false;
  private wakeLock?: WakeLockSentinel;
  private wakeLockReleaseSubscription?: Subscription;

  get locked() {
    return this.wakeLock?.released === false;
  }

  async lock() {
    this.wantLocked = true;
    if ('wakeLock' in navigator) {
      this.wakeLockReleaseSubscription?.unsubscribe();
      await this.wakeLock?.release();
      this.wakeLock = await navigator.wakeLock.request('screen');
      // If we unlocked across the await here, release the lock we just acquired
      // and return
      if (!this.wantLocked) {
        await this.wakeLock.release();
        return;
      }

      this.wakeLockReleaseSubscription = fromEvent(
        this.wakeLock,
        'release',
      ).subscribe(() => {
        if (this.wantLocked) {
          console.warn(`Wake lock was released unexpectedly, re-acquiring`);
          this.lock().catch((e) => {
            console.error('Failed to re-acquire wake lock', e);
          });
        }
      });
    } else {
      throw new Error('Wake Lock API not supported');
    }
  }

  unlock() {
    this.wantLocked = false;
    this.wakeLock?.release();
    this.wakeLockReleaseSubscription?.unsubscribe();
    this.wakeLock = undefined;
    this.wakeLockReleaseSubscription = undefined;
  }
}
