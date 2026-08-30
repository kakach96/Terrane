import { Component } from '@angular/core';
import { MatDialog } from '@angular/material/dialog';
import { AuthService } from './services/auth.service';
import { LanguageService, SupportedLanguage } from './services/language.service';
import { LoginComponent } from './components/login/login.component';

@Component({
  standalone: false,
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss'],
})
export class AppComponent {
  title = 'Terrane';
  sidenavOpened = true;
  currentLang = this.languageService.currentLang;

  // Navigation group expansion state (mirrors GeoServer menu grouping)
  private navGroups: { [key: string]: boolean } = {
    services: true,
    data: true,
    tiles: true,
    server: true,
    security: true,
  };

  isGroupOpen(group: string): boolean {
    return !!this.navGroups[group];
  }

  toggleGroup(group: string): void {
    this.navGroups[group] = !this.navGroups[group];
  }

  constructor(
    public auth: AuthService,
    private dialog: MatDialog,
    public languageService: LanguageService,
  ) {}

  setLanguage(lang: SupportedLanguage): void {
    this.languageService.setLanguage(lang);
  }

  openLoginDialog(): void {
    this.dialog.open(LoginComponent, {
      width: '420px',
    });
  }

  logout(): void {
    this.auth.logout();
    this.openLoginDialog();
  }
}
