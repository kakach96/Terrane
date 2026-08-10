import { Component, OnInit } from '@angular/core';
import { Router, NavigationEnd } from '@angular/router';
import { filter } from 'rxjs/operators';
import { MatDialog } from '@angular/material/dialog';
import { AuthService } from './services/auth.service';
import { LoginComponent } from './components/login/login.component';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss'],
})
export class AppComponent implements OnInit {
  title = 'Terrane';
  pageTitle = '图层预览';
  sidenavOpened = true;

  private menuTitles: { [key: string]: string } = {
    '/services': '服务概览',
    '/workspaces': '工作空间',
    '/data-sources': '数据源',
    '/layers': '图层',
    '/layer-preview': '图层预览',
    '/tile-layers': '切片图层',
    '/layer-groups': '图层组',
    '/styles': '样式管理',
    '/monitor': '监控面板',
    '/users': '用户管理',
    '/permissions': '权限管理',
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
  ) {}

  ngOnInit(): void {
    this.router.events
      .pipe(filter((event): event is NavigationEnd => event instanceof NavigationEnd))
      .subscribe((event) => {
        this.updatePageTitle(event.url);
      });

    this.updatePageTitle(this.router.url);
  }

  private updatePageTitle(url: string): void {
    const basePath = url.split('?')[0];
    this.pageTitle = this.menuTitles[basePath] || 'Terrane';
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
