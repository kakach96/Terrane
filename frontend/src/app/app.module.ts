import { NgModule } from '@angular/core';
import { BrowserModule } from '@angular/platform-browser';
import { HttpClientModule } from '@angular/common/http';
import { BrowserAnimationsModule } from '@angular/platform-browser/animations';
import { FormsModule, ReactiveFormsModule } from '@angular/forms';
import { RouterModule, Routes } from '@angular/router';

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
import { DashboardComponent } from './components/dashboard/dashboard.component';
import { LayersComponent } from './components/layers/layers.component';
import { LayerDetailComponent } from './components/layer-detail/layer-detail.component';
import { FeatureDetailDialogComponent } from './components/layer-detail/feature-detail-dialog.component';
import { LayerCreateComponent } from './components/layer-create/layer-create.component';
import { PreviewComponent } from './components/preview/preview.component';
import { ConfirmDialogComponent } from './shared/components/confirm-dialog.component';
import { WorkspacesComponent } from './components/workspaces/workspaces.component';
import { WorkspaceDialogComponent } from './components/workspaces/workspace-dialog.component';
import { DataSourcesComponent } from './components/data-sources/data-sources.component';
import { DataSourceDialogComponent } from './components/data-sources/data-source-dialog/data-source-dialog.component';
import { TileLayersComponent } from './components/tile-layers/tile-layers.component';
import { ServerStatusComponent } from './components/server-status/server-status.component';
import { StylesComponent } from './components/styles/styles.component';
import { StyleEditorDialogComponent } from './components/styles/style-editor-dialog.component';
import { LayerGroupsComponent } from './components/layer-groups/layer-groups.component';
import { CreateLayerGroupDialogComponent } from './components/layer-groups/create-layer-group-dialog.component';
import { NamespacesComponent } from './components/namespaces/namespaces.component';
import { NamespaceDialogComponent } from './components/namespaces/namespace-dialog.component';
import { StoresComponent } from './components/stores/stores.component';

const routes: Routes = [
  { path: '', redirectTo: '/dashboard', pathMatch: 'full' },
  { path: 'dashboard', component: DashboardComponent },
  { path: 'workspaces', component: WorkspacesComponent },
  { path: 'namespaces', component: NamespacesComponent },
  { path: 'data-sources', component: DataSourcesComponent },
  { path: 'stores', component: StoresComponent },
  { path: 'layers', component: LayersComponent },
  { path: 'layers/create', component: LayerCreateComponent },
  { path: 'layers/:name', component: LayerDetailComponent },
  { path: 'layer-preview', component: PreviewComponent },
  { path: 'layer-groups', component: LayerGroupsComponent },
  { path: 'styles', component: StylesComponent },
  { path: 'tile-layers', component: TileLayersComponent },
  { path: 'server-status', component: ServerStatusComponent },
];

@NgModule({
  declarations: [
    AppComponent,
    DashboardComponent,
    LayersComponent,
    LayerDetailComponent,
    LayerCreateComponent,
    PreviewComponent,
    ConfirmDialogComponent,
    WorkspacesComponent,
    WorkspaceDialogComponent,
    DataSourcesComponent,
    DataSourceDialogComponent,
    TileLayersComponent,
    ServerStatusComponent,
    StylesComponent,
    StyleEditorDialogComponent,
    LayerGroupsComponent,
    CreateLayerGroupDialogComponent,
    NamespacesComponent,
    NamespaceDialogComponent,
    StoresComponent
  ],
  imports: [
    BrowserModule,
    BrowserAnimationsModule,
    HttpClientModule,
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
    MatChipsModule,
    MatTooltipModule,
    MatDividerModule,
    MatMenuModule,
    MatBadgeModule,
    MatRippleModule,
    MatSlideToggleModule,
    MatCheckboxModule,
    MatButtonToggleModule,
    FeatureDetailDialogComponent
  ],
  providers: [],
  bootstrap: [AppComponent]
})
export class AppModule { }
