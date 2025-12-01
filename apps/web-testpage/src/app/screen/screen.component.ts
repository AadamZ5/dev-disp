import {
  AfterViewInit,
  Component,
  computed,
  effect,
  ElementRef,
  inject,
  INJECTOR,
  viewChild,
} from '@angular/core';
import {
  asyncScheduler,
  distinctUntilChanged,
  map,
  OperatorFunction,
  retry,
  scan,
  share,
  shareReplay,
  switchMap,
  tap,
} from 'rxjs';
import { DevDispService, fromDevDispConnection } from '../dev-disp.service';
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
export class ScreenComponent implements AfterViewInit {
  private readonly injector = inject(INJECTOR);
  private readonly devDispService = inject(DevDispService);
  readonly canvas = viewChild<ElementRef<HTMLCanvasElement>>('screen');

  readonly offscreenCanvas$ = toObservable(this.canvas).pipe(
    distinctUntilChanged(),
    map((canvas) => canvas?.nativeElement.transferControlToOffscreen()),
    distinctUntilChanged(),
    shareReplay(1)
  );

  // TODO: Correctly display data
  readonly data$ = this.offscreenCanvas$.pipe(
    switchMap((offscreenCanvas) =>
      fromDevDispConnection(() =>
        this.devDispService.connect(
          `${window.location.hostname}:56789`,
          offscreenCanvas
        )
      )
    ),

    tap({
      error: (e) => {
        console.error('Dev-disp connection error', e);
      },
    }),
    retry({ delay: 5000 }),
    share()
  );

  readonly dataEpoch = toSignal(
    this.data$.pipe(
      scan((acc) => {
        return acc + 1;
      }, 0)
    ),
    { initialValue: -1 }
  );

  readonly fps = toSignal(
    this.data$.pipe(
      map(() => performance.now()),
      slidingWindow(30),
      map((times) => {
        if (times.length < 2) {
          return 0;
        }
        const duration = times[0] - times[times.length - 1];
        return ((times.length - 1) * 1000) / duration;
      })
    ),
    { initialValue: 0 }
  );

  ngAfterViewInit(): void {
    asyncScheduler.schedule(() => {
      effect(
        () => {
          const canvas = this.canvas();

          const ctx = canvas?.nativeElement.getContext('2d');
          if (ctx) {
            ctx.fillStyle = 'green';
            ctx.fillRect(
              0,
              0,
              canvas!.nativeElement.width,
              canvas!.nativeElement.height
            );
          }
        },
        {
          injector: this.injector,
        }
      );
    });
  }
}
