import { Component } from '@angular/core';
import { RouterModule } from '@angular/router';
import { ScreenComponent } from './screen/screen.component';
import { ReactiveFormsModule } from '@angular/forms';
import { CommonModule } from '@angular/common';

@Component({
  imports: [RouterModule, CommonModule, ScreenComponent, ReactiveFormsModule],
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class App {
  protected title = 'web-testpage';
}
