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
import { DevDispService } from '../dev-disp.service';

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

  readonly connection = this.devDispService.connect('ws://localhost:56789');

  // TODO: This is temporary test code to provide basic responses to the
  // server. Replace with actual implementation later. Probably replace
  // with WASM-based library
  readonly dataSub = this.connection.anyData$.subscribe((data) => {
    const view = new Uint8Array(data);

    console.log('Received data from dev-disp:', view);

    switch (view[0]) {
      case 0x00: {
        // Pre-init request. Respond with 0
        const response = new Uint8Array([0x00]);
        this.connection.send(response.buffer);
        console.log('Sent pre-init response to dev-disp:', response);
        break;
      }
      case 0x01: {
        const response = new Uint8Array([0x01, 0x01, 0x00, 0x00, 0x00, 0x00]);
        this.connection.send(response.buffer);
        console.log('Sent device info response to dev-disp:', response);
        break;
      }
      default: {
        console.log('Unknown message type from dev-disp:', view[0]);
      }
    }
  });

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
