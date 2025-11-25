import {
  AfterViewInit,
  Component,
  effect,
  ElementRef,
  inject,
  INJECTOR,
  viewChild,
} from '@angular/core';
import { asyncScheduler } from 'rxjs';

@Component({
  selector: 'app-screen',
  imports: [],
  templateUrl: './screen.component.html',
  styleUrl: './screen.component.scss',
})
export class ScreenComponent implements AfterViewInit {
  private readonly injector = inject(INJECTOR);
  readonly canvas = viewChild<ElementRef<HTMLCanvasElement>>('screen');

  constructor() {}

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
