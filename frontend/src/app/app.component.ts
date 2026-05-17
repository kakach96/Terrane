import { Component, OnInit } from '@angular/core';
import { Router, NavigationEnd } from '@angular/router';
import { filter } from 'rxjs/operators';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss']
})
export class AppComponent implements OnInit {
  title = 'RRGeoServer';
  pageTitle = '仪表盘';
  sidenavOpened = true;

  private menuTitles: { [key: string]: string } = {
    '/dashboard': '仪表盘',
    '/workspaces': '工作空间',
    '/data-sources': '数据源',
    '/layers': '图层',
    '/layer-preview': '图层预览',
    '/tile-layers': '切片图层',
    '/server-status': '服务器状态'
  };

  constructor(private router: Router) {}

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
    this.pageTitle = this.menuTitles[basePath] || 'RRGeoServer';
  }
}
