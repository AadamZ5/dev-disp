import {
  AfterViewInit,
  Component,
  effect,
  ElementRef,
  inject,
  INJECTOR,
  viewChild,
} from '@angular/core';
import {
  asyncScheduler,
  map,
  Observable,
  OperatorFunction,
  retry,
  scan,
} from 'rxjs';
import { DevDispService, fromDevDispConnection } from '../dev-disp.service';
import { toSignal } from '@angular/core/rxjs-interop';

function bufferRing<T>(size: number): OperatorFunction<T, T[]> {
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

  readonly data$ = fromDevDispConnection(() =>
    this.devDispService.connect('ws://localhost:56789')
  ).pipe(retry({ delay: 5000 }));

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
      bufferRing(30),
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

  // TODO: This is temporary test code to provide basic responses to the
  // server. Replace with actual implementation later. Probably replace
  // with WASM-based library

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
