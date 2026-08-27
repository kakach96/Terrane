import { Component, computed } from '@angular/core';
import { Router, NavigationEnd } from '@angular/router';
import { filter, map } from 'rxjs/operators';
import { toSignal } from '@angular/core/rxjs-interop';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
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

  // Translate keys for the toolbar page title (indexed by route path).
  private menuTitleKeys: { [key: string]: string } = {
    '/services': 'nav.servicesOverview',
    '/workspaces': 'nav.workspaces',
    '/data-sources': 'nav.dataSources',
    '/layers': 'nav.layers',
    '/layer-preview': 'nav.layerPreview',
    '/tile-layers': 'nav.tileLayers',
    '/layer-groups': 'nav.layerGroups',
    '/styles': 'nav.styles',
    '/monitor': 'nav.monitor',
    '/users': 'nav.users',
    '/permissions': 'nav.permissions',
  };

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

  private currentUrl = toSignal(
    this.router.events.pipe(
      filter((event): event is NavigationEnd => event instanceof NavigationEnd),
      map(() => this.router.url),
    ),
    { initialValue: this.router.url },
  );

  /** Toolbar page title, reactive to both route changes and language switches. */
  pageTitle = computed(() => {
    // Read currentLang to establish a dependency so the title re-translates on switch.
    this.currentLang();
    const basePath = this.currentUrl()?.split('?')[0] ?? '';
    const key = this.menuTitleKeys[basePath];
    return key ? this.translate.instant(key) : 'Terrane';
  });

  constructor(
    private router: Router,
    public auth: AuthService,
    private dialog: MatDialog,
    private translate: TranslateService,
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
