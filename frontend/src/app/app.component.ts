import { Component, OnInit } from '@angular/core';
import { Router, NavigationEnd } from '@angular/router';
import { filter } from 'rxjs/operators';
import { MatDialog } from '@angular/material/dialog';
import { AuthService } from './services/auth.service';
import { LoginComponent } from './components/login/login.component';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss']
})
export class AppComponent implements OnInit {
  title = 'Terrane';
  pageTitle = '仪表盘';
  sidenavOpened = true;

  private menuTitles: { [key: string]: string } = {
    '/dashboard': '仪表盘',
    '/workspaces': '工作空间',
    '/namespaces': '命名空间',
    '/data-sources': '数据源',
    '/stores': '存储管理',
    '/layers': '图层',
    '/layer-preview': '图层预览',
    '/tile-layers': '切片图层',
    '/layer-groups': '图层组',
    '/styles': '样式管理',
    '/server-status': '服务器状态',
    '/monitor': '监控面板',
    '/users': '用户管理',
    '/permissions': '权限管理',
  };

  constructor(
    private router: Router,
    public auth: AuthService,
    private dialog: MatDialog,
  ) {}

  ngOnInit(): void {
    this.router.events.pipe(
      filter((event): event is NavigationEnd => event instanceof NavigationEnd)
    ).subscribe((event) => {
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
