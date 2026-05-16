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

import { AppComponent } from './app.component';
import { DashboardComponent } from './components/dashboard/dashboard.component';
import { LayersComponent } from './components/layers/layers.component';
import { LayerDetailComponent } from './components/layer-detail/layer-detail.component';
import { LayerCreateComponent } from './components/layer-create/layer-create.component';
import { PreviewComponent } from './components/preview/preview.component';
import { ConfirmDialogComponent } from './shared/components/confirm-dialog.component';
import { WorkspacesComponent } from './components/workspaces/workspaces.component';
import { WorkspaceDialogComponent } from './components/workspaces/workspace-dialog.component';
import { DatasourcesComponent } from './components/datasources/datasources.component';
import { TileLayersComponent } from './components/tile-layers/tile-layers.component';
import { ServerStatusComponent } from './components/server-status/server-status.component';

const routes: Routes = [
  { path: '', redirectTo: '/dashboard', pathMatch: 'full' },
  { path: 'dashboard', component: DashboardComponent },
  { path: 'workspaces', component: WorkspacesComponent },
  { path: 'layers', component: LayersComponent },
  { path: 'layers/create', component: LayerCreateComponent },
  { path: 'layers/:name', component: LayerDetailComponent },
  { path: 'layer-preview', component: PreviewComponent },
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
    DatasourcesComponent,
    TileLayersComponent,
    ServerStatusComponent
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
    MatRippleModule
  ],
  providers: [],
  bootstrap: [AppComponent]
})
export class AppModule { }
