import { NgModule } from '@angular/core';
import { BrowserModule } from '@angular/platform-browser';
import { provideHttpClient, withInterceptorsFromDi, withXhr, HTTP_INTERCEPTORS } from '@angular/common/http';
import { BrowserAnimationsModule } from '@angular/platform-browser/animations';
import { FormsModule, ReactiveFormsModule } from '@angular/forms';
import { RouterModule, Routes } from '@angular/router';
import { TranslatePipe, provideTranslateService } from '@ngx-translate/core';
import { provideTranslateHttpLoader } from '@ngx-translate/http-loader';

import { MatToolbarModule } from '@angular/material/toolbar';
import { MatSidenavModule } from '@angular/material/sidenav';
import { MatListModule } from '@angular/material/list';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatCardModule } from '@angular/material/card';
import { MatTableModule } from '@angular/material/table';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatDialogModule } from '@angular/material/dialog';
import { MatSnackBarModule } from '@angular/material/snack-bar';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatChipsModule } from '@angular/material/chips';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { MatMenuModule } from '@angular/material/menu';
import { MatBadgeModule } from '@angular/material/badge';
import { MatRippleModule } from '@angular/material/core';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatButtonToggleModule } from '@angular/material/button-toggle';

import { AppComponent } from './app.component';
import { ServicesComponent } from './components/services/services.component';
import { LayersComponent } from './components/layers/layers.component';
import { LayerDetailComponent } from './components/layer-detail/layer-detail.component';
import { LayerCreateComponent } from './components/layer-create/layer-create.component';
import { PreviewComponent } from './components/preview/preview.component';
import { ConfirmDialogComponent } from './shared/components/confirm-dialog.component';
import { WorkspacesComponent } from './components/workspaces/workspaces.component';
import { WorkspaceDialogComponent } from './components/workspaces/workspace-dialog.component';
import { DataSourcesComponent } from './components/data-sources/data-sources.component';
import { DataSourceDialogComponent } from './components/data-sources/data-source-dialog/data-source-dialog.component';
import { DirectoryBrowserComponent } from './components/shared/directory-browser/directory-browser.component';
import { TileLayersComponent } from './components/tile-layers/tile-layers.component';
import { StylesComponent } from './components/styles/styles.component';
import { StyleEditorDialogComponent } from './components/styles/style-editor-dialog.component';
import { LayerGroupsComponent } from './components/layer-groups/layer-groups.component';
import { CreateLayerGroupDialogComponent } from './components/layer-groups/create-layer-group-dialog.component';
import { LoginComponent } from './components/login/login.component';
import { MonitorComponent } from './components/monitor/monitor.component';
import { UsersComponent } from './components/users/users.component';
import { PermissionsComponent } from './components/permissions/permissions.component';
import { AuthInterceptor } from './services/auth.interceptor';

const routes: Routes = [
  { path: '', redirectTo: '/services', pathMatch: 'full' },
  { path: 'services', component: ServicesComponent },
  { path: 'workspaces', component: WorkspacesComponent },
  { path: 'data-sources', component: DataSourcesComponent },
  { path: 'layers', component: LayersComponent },
  { path: 'layers/create', component: LayerCreateComponent },
  { path: 'layers/:name', component: LayerDetailComponent },
  { path: 'layer-preview', component: PreviewComponent },
  { path: 'layer-groups', component: LayerGroupsComponent },
  { path: 'styles', component: StylesComponent },
  { path: 'tile-layers', component: TileLayersComponent },
  { path: 'monitor', component: MonitorComponent },
  { path: 'users', component: UsersComponent },
  { path: 'permissions', component: PermissionsComponent },
];

@NgModule({
  declarations: [
    AppComponent,
    ServicesComponent,
    LayersComponent,
    LayerDetailComponent,
    LayerCreateComponent,
    PreviewComponent,
    ConfirmDialogComponent,
    WorkspacesComponent,
    WorkspaceDialogComponent,
    DataSourcesComponent,
    DataSourceDialogComponent,
    DirectoryBrowserComponent,
    TileLayersComponent,
    StylesComponent,
    StyleEditorDialogComponent,
    LayerGroupsComponent,
    CreateLayerGroupDialogComponent,
    LoginComponent,
    MonitorComponent,
    UsersComponent,
    PermissionsComponent,
  ],
  providers: [
    { provide: HTTP_INTERCEPTORS, useClass: AuthInterceptor, multi: true },
    provideHttpClient(withInterceptorsFromDi(), withXhr()),
    provideTranslateService({ lang: 'zh-CN' }),
    provideTranslateHttpLoader({ prefix: './assets/i18n/', suffix: '.json' }),
  ],
  imports: [
    BrowserModule,
    BrowserAnimationsModule,
    TranslatePipe,
    FormsModule,
    ReactiveFormsModule,
    RouterModule.forRoot(routes),
    MatToolbarModule,
    MatSidenavModule,
    MatListModule,
    MatIconModule,
    MatButtonModule,
    MatCardModule,
    MatTableModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    MatDialogModule,
    MatSnackBarModule,
    MatProgressSpinnerModule,
    MatProgressBarModule,
    MatChipsModule,
    MatTooltipModule,
    MatDividerModule,
    MatMenuModule,
    MatBadgeModule,
    MatRippleModule,
    MatSlideToggleModule,
    MatCheckboxModule,
    MatButtonToggleModule,
  ],
  bootstrap: [AppComponent],
})
export class AppModule {}
