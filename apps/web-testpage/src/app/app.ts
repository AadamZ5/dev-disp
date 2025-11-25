import { Component } from '@angular/core';
import { RouterModule } from '@angular/router';
import { ScreenComponent } from './screen/screen.component';
import { ReactiveFormsModule } from '@angular/forms';

@Component({
  imports: [RouterModule, ScreenComponent, ReactiveFormsModule],
  selector: 'app-root',
  templateUrl: './app.html',
  styleUrl: './app.scss',
})
export class App {
  protected title = 'web-testpage';
}
