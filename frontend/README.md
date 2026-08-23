# Terrane Frontend

A modern Terrane admin interface built with Angular 17 and Angular Material.

## 🚀 Quick Start

### 1. Install Dependencies

```bash
cd frontend
npm install
```

### 2. Start the Dev Server

```bash
ng serve
```

Visit **http://localhost:4200** (the dev server automatically proxies `/api` requests to the backend)

### 3. Start the Backend Server

In another terminal:

```bash
cd ..\service  # enter the backend service directory
cargo run
```

The backend runs on **http://localhost:8080**

## 📁 Project Structure

```
frontend/
├── src/
│   ├── app/
│   │   ├── components/           # Page components
│   │   │   ├── dashboard/        # 📊 Dashboard
│   │   │   ├── layers/           # 📚 Layer list
│   │   │   ├── layer-detail/     # 🔍 Layer detail
│   │   │   ├── layer-create/     # ➕ Create layer
│   │   │   ├── preview/          # 🖼️ Preview component
│   │   │   ├── workspaces/       # 🗂️ Workspace management
│   │   │   ├── namespaces/       # 🏷️ Namespace management
│   │   │   ├── stores/           # 🗄️ Store management
│   │   │   ├── data-sources/     # 🔌 Data source management
│   │   │   ├── styles/           # 🎨 Style management (SLD/CSS/YSLD/MBStyle)
│   │   │   ├── layer-groups/     # 📚 Layer group management
│   │   │   ├── tile-layers/      # 🧩 Tile layers + GeoWebCache stats
│   │   │   ├── monitor/          # 📈 Monitoring
│   │   │   ├── server-status/    # 🖥️ Server status
│   │   │   ├── login/            # 🔐 Login
│   │   │   ├── users/            # 👥 User management
│   │   │   └── permissions/      # 🛡️ Permissions
│   │   ├── services/             # 🔧 Business services
│   │   │   ├── geoserver.service.ts      # GeoServer API
│   │   │   └── notification.service.ts   # Notification service
│   │   ├── models/               # 📦 Data models
│   │   │   └── geoserver.models.ts
│   │   ├── shared/               # 🔄 Shared components
│   │   │   └── components/
│   │   │       └── confirm-dialog.component.ts
│   │   ├── app.component.ts      # Root component
│   │   ├── app.module.ts          # Root module
│   │   └── app-routing.module.ts  # Routing configuration
│   ├── styles.scss                # Global styles
│   ├── index.html                 # HTML entry
│   └── main.ts                    # Application entry
├── angular.json                   # Angular configuration
├── package.json                   # Dependency configuration
├── tsconfig.json                  # TypeScript configuration
├── proxy.conf.json                # Dev proxy configuration
└── README.md                      # Project documentation
```

## 🎯 Feature Modules

### 1. Dashboard (`/dashboard`)
- System statistics overview
- Recent layer list
- Quick action entries

### 2. Layer Management (`/layers`)
- Layer list display
- Search and filtering
- Layer card view
- Delete layer

### 3. Create Layer (`/layers/create`)
- Form validation
- Workspace selection
- Coordinate reference system configuration
- Boundary configuration

### 4. Layer Detail (`/layers/:name`)
- Layer info display
- Live preview
- Feature browsing (read-only list + GeoJSON/CSV export)
- Preview size adjustment

## 🎨 Design Features

### UI/UX
- **Material Design 3** - follows the Material Design spec
- **Responsive layout** - supports desktop and mobile devices
- **Animations** - smooth transition animations
- **Dark sidebar** - professional data-management interface style

### Technical Highlights
- **Modular architecture** - clear project structure
- **RxJS** - reactive programming
- **TypeScript** - type safety
- **SCSS** - modern style management

## 🔌 API Integration

The frontend communicates with the backend via Angular HttpClient. The API base path is
`/geoserver` (matching the backend `api_context`, see [DEVELOPMENT.md](../docs/DEVELOPMENT.md)), e.g.:

| Method | Endpoint | Description |
|------|------|------|
| GET | `/geoserver/layers` | Get all layers |
| POST | `/geoserver/layers` | Create a new layer |
| GET | `/geoserver/layers/:name` | Get layer details |
| PUT | `/geoserver/layers/:name` | Update a layer |
| DELETE | `/geoserver/layers/:name` | Delete a layer |
| GET | `/geoserver/layers/:name/preview` | Get layer preview image |
| GET | `/geoserver/layers/:name/features` | Get layer features (read-only) |

## 🛠️ Dev Commands

```bash
# Dev server
ng serve

# Build production version
ng build

# Run tests
ng test

# Watch-mode build (lazy builds)
ng build --watch --configuration development
```

## 📦 Extension Guide

### Adding a New Page

1. Create a component directory under `src/app/components/`
2. Create the `.ts`, `.html`, `.scss` files
3. Declare the component in `app.module.ts`
4. Add the route to the routing configuration

### Adding a New Service

1. Create a service file under `src/app/services/`
2. Use the `@Injectable({ providedIn: 'root' })` decorator
3. Inject and use it in components via the constructor

### Adding a New Model

1. Create a model file under `src/app/models/`
2. Export a TypeScript interface or class
3. Import and use it where needed

## 🎨 Custom Theme

Edit `src/styles.scss` to modify the theme configuration:

```scss
@use '@angular/material' as mat;

$geoserver-primary: mat.m2-define-palette(mat.$m2-indigo-palette, 700, 500, 900);
$geoserver-accent: mat.m2-define-palette(mat.$m2-teal-palette, A400, A200, A700);

$geoserver-theme: mat.m2-define-light-theme((
  color: (
    primary: $geoserver-primary,
    accent: $geoserver-accent,
  ),
));
```

## 📝 Notes

1. **Node.js version** - requires Node.js 18.x or higher
2. **Angular CLI** - install globally: `npm install -g @angular/cli`
3. **Proxy configuration** - API requests are automatically proxied to the backend in development
4. **CORS** - ensure the backend allows cross-origin requests

## 🚀 Deployment

### Development Environment
```bash
ng serve
```

### Production Build
```bash
ng build --configuration production
```

The build output is in `dist/terrane-ui/`

### Integrating with the Backend

Copy the `dist/terrane-ui/` directory to the backend project's static file directory, and configure the server to serve those files.

## 📚 Related Documentation

- [Development guide (backend + full setup)](../docs/DEVELOPMENT.md)
- [Architecture](../docs/ARCHITECTURE.md)
- [Roadmap](../docs/ROADMAP.md)

### Cloud-Native Deployment

- **Multi-stage container build (recommended)**: run `npm ci && ng build --configuration production` in the Docker `node` stage; the `dist/terrane-ui/` output is bundled into the runtime image alongside the Rust binary and served from the backend `static/` directory — **no separate frontend service needed** (see section 6 of `docs/IMPLEMENTATION_PLAN.md`).
- **Standalone NGINX container (optional)**: serve the static assets separately and reverse-proxy `/api`, `/geoserver`, `/wms`, `/wfs`, `/wcs`, `/tiles` to the backend service.
- The build output defaults to `dist/terrane-ui/`.

## 📚 Learning Resources

- [Angular official docs](https://angular.io/docs)
- [Angular Material component library](https://material.angular.io/)
- [TypeScript handbook](https://www.typescriptlang.org/docs/)
- [RxJS documentation](https://rxjs.dev/guide/overview)
