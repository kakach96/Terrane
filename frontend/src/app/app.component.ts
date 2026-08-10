import { Component, OnInit, OnDestroy } from '@angular/core';
import { Router, NavigationEnd } from '@angular/router';
import { filter } from 'rxjs/operators';
import { Subscription } from 'rxjs';
import { MatDialog } from '@angular/material/dialog';
import { TranslateService } from '@ngx-translate/core';
import { AuthService } from './services/auth.service';
import { LanguageService, SupportedLanguage } from './services/language.service';
import { LoginComponent } from './components/login/login.component';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss'],
})
export class AppComponent implements OnInit, OnDestroy {
  title = 'Terrane';
  pageTitle = 'Terrane';
  sidenavOpened = true;
  currentLang: SupportedLanguage = 'zh-CN';
  private langSub!: Subscription;

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

  // 导航分组展开状态（对标 GeoServer 菜单分组）
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
    private router: Router,
    public auth: AuthService,
    private dialog: MatDialog,
    private translate: TranslateService,
    public languageService: LanguageService,
  ) {}

  ngOnInit(): void {
    this.currentLang = this.languageService.currentLang;
    this.langSub = this.translate.onLangChange.subscribe((event) => {
      this.currentLang = event.lang as SupportedLanguage;
      this.updatePageTitle(this.router.url);
    });

    this.router.events
      .pipe(filter((event): event is NavigationEnd => event instanceof NavigationEnd))
      .subscribe((event) => {
        this.updatePageTitle(event.url);
      });

    this.updatePageTitle(this.router.url);
  }

  ngOnDestroy(): void {
    this.langSub?.unsubscribe();
  }

  setLanguage(lang: SupportedLanguage): void {
    this.languageService.setLanguage(lang);
  }

  private updatePageTitle(url: string): void {
    const basePath = url.split('?')[0];
    const key = this.menuTitleKeys[basePath];
    this.pageTitle = key ? this.translate.instant(key) : 'Terrane';
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
