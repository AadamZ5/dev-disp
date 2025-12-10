import { Component, ElementRef, inject, viewChild } from '@angular/core';
import {
  distinctUntilChanged,
  endWith,
  map,
  OperatorFunction,
  retry,
  scan,
  share,
  shareReplay,
  switchMap,
  tap,
  throttleTime,
} from 'rxjs';
import { DevDispService, ofDevDispConnection } from '../dev-disp.service';
import { toObservable, toSignal } from '@angular/core/rxjs-interop';

// TODO: Move to a shared utilities library
function slidingWindow<T>(size: number): OperatorFunction<T, T[]> {
  return scan((acc: T[], value: T) => {
    acc.unshift(value);
    if (acc.length > size) {
      acc.pop();
    }
    return acc;
  }, [] as T[]);
}

@Component({
  selector: 'app-screen',
  imports: [],
  templateUrl: './screen.component.html',
  styleUrl: './screen.component.scss',
})
export class ScreenComponent {
  private readonly devDispService = inject(DevDispService);
  readonly canvas = viewChild<ElementRef<HTMLCanvasElement>>('screen');

  readonly offscreenCanvas$ = toObservable(this.canvas).pipe(
    distinctUntilChanged(),
    map((canvas) => {
      const offscreeen = canvas?.nativeElement.transferControlToOffscreen();
      if (offscreeen && canvas) {
        offscreeen.width = canvas.nativeElement.clientWidth;
        offscreeen.height = canvas.nativeElement.clientHeight;
      }
      return offscreeen;
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
}
