# Solution 3: Flutter Web — Production Architecture

## The Problem

Build a Flutter web application that performs well, is SEO-friendly where needed, loads fast, and works across browsers — while sharing code with mobile if applicable.

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│              Flutter Web App                     │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │           Presentation Layer              │   │
│  │  ┌────────────┐  ┌────────────────────┐  │   │
│  │  │ Responsive │  │  Web-Specific      │  │   │
│  │  │ Layouts    │  │  (URL strategy,    │  │   │
│  │  │ (mobile/   │  │   browser APIs,    │  │   │
│  │  │  tablet/   │  │   SEO meta tags)   │  │   │
│  │  │  desktop)  │  │                    │  │   │
│  │  └────────────┘  └────────────────────┘  │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │           Business Logic                  │   │
│  │  (Shared with mobile — platform agnostic) │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │           Platform Abstraction            │   │
│  │  ┌─────────┐ ┌──────────┐ ┌──────────┐  │   │
│  │  │ Web     │ │ Mobile   │ │ Desktop  │  │   │
│  │  │ Storage │ │ Storage  │ │ Storage  │  │   │
│  │  │ (local  │ │ (shared  │ │ (file)   │  │   │
│  │  │  store) │ │  prefs)  │ │          │  │   │
│  │  └─────────┘ └──────────┘ └──────────┘  │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

## Project Structure

```
flutter_web_app/
├── pubspec.yaml
├── web/
│   ├── index.html              # Custom HTML shell
│   ├── manifest.json           # PWA manifest
│   ├── favicon.ico
│   └── icons/
├── lib/
│   ├── main.dart
│   ├── app.dart
│   ├── config/
│   │   ├── router.dart         # GoRouter with URL strategy
│   │   ├── theme.dart
│   │   └── env.dart            # Environment config
│   ├── features/
│   │   ├── home/
│   │   ├── dashboard/
│   │   └── settings/
│   ├── shared/
│   │   ├── widgets/
│   │   │   ├── responsive_layout.dart
│   │   │   ├── web_scaffold.dart
│   │   │   └── seo_head.dart
│   │   ├── platform/
│   │   │   ├── storage_service.dart       # Abstract
│   │   │   ├── storage_service_web.dart   # Web impl
│   │   │   └── storage_service_mobile.dart
│   │   └── utils/
│   │       ├── web_utils.dart
│   │       └── url_launcher_web.dart
│   └── l10n/
└── test/
```

## Key Technical Decisions

### 1. Renderer Selection

```dart
// web/index.html
<script>
  // Use CanvasKit for rich graphics, WASM for best performance
  // Use HTML renderer for text-heavy apps with SEO needs
  
  const renderer = new URLSearchParams(window.location.search).get('renderer');
  
  _flutter.loader.load({
    config: {
      // WASM for modern browsers, canvaskit fallback, html for SEO pages
      renderer: renderer || 'canvaskit',
    },
  });
</script>
```

**Decision matrix:**

| Renderer | Best For | Trade-off |
|---|---|---|
| **CanvasKit** | Rich UI, animations, consistency with mobile | Larger initial load (~2MB WASM) |
| **HTML** | Text-heavy, SEO-important, fast initial load | Inconsistent rendering, missing some widgets |
| **WASM (Skwasm)** | Best performance, Flutter 3.22+ | Requires modern browser, largest binary |

### 2. Responsive Design System

```dart
// lib/shared/widgets/responsive_layout.dart

enum ScreenSize { mobile, tablet, desktop }

class ResponsiveLayout extends StatelessWidget {
  final Widget mobile;
  final Widget? tablet;
  final Widget desktop;

  const ResponsiveLayout({
    required this.mobile,
    this.tablet,
    required this.desktop,
  });

  static ScreenSize of(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width;
    if (width < 600) return ScreenSize.mobile;
    if (width < 1200) return ScreenSize.tablet;
    return ScreenSize.desktop;
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, constraints) {
      if (constraints.maxWidth >= 1200) return desktop;
      if (constraints.maxWidth >= 600) return tablet ?? desktop;
      return mobile;
    });
  }
}

// Usage:
ResponsiveLayout(
  mobile: MobileHomePage(),
  tablet: TabletHomePage(),
  desktop: DesktopHomePage(),
)
```

### 3. URL Strategy & Deep Linking

```dart
// lib/config/router.dart

final router = GoRouter(
  // Use path-based URLs (not hash)
  // Requires server-side config for SPA fallback
  initialLocation: '/',
  routes: [
    GoRoute(path: '/', builder: (_, __) => const HomePage()),
    GoRoute(path: '/dashboard', builder: (_, __) => const DashboardPage()),
    GoRoute(
      path: '/products/:id',
      builder: (_, state) => ProductPage(id: state.pathParameters['id']!),
    ),
    GoRoute(
      path: '/search',
      builder: (_, state) => SearchPage(
        query: state.uri.queryParameters['q'] ?? '',
      ),
    ),
  ],
  errorBuilder: (_, __) => const NotFoundPage(),
);
```

**Server config for SPA (nginx):**
```nginx
location / {
    try_files $uri $uri/ /index.html;
}
```

### 4. SEO & Meta Tags

```dart
// lib/shared/widgets/seo_head.dart
import 'dart:html' as html;

class SeoHead {
  static void update({
    required String title,
    required String description,
    String? image,
    String? url,
  }) {
    html.document.title = title;
    _setMeta('description', description);
    _setMeta('og:title', title);
    _setMeta('og:description', description);
    if (image != null) _setMeta('og:image', image);
    if (url != null) _setMeta('og:url', url);
  }

  static void _setMeta(String name, String content) {
    var meta = html.document.head?.querySelector('meta[property="$name"]')
        ?? html.document.head?.querySelector('meta[name="$name"]');
    if (meta == null) {
      meta = html.document.createElement('meta');
      if (name.startsWith('og:')) {
        meta.setAttribute('property', name);
      } else {
        meta.setAttribute('name', name);
      }
      html.document.head?.append(meta);
    }
    meta.setAttribute('content', content);
  }
}
```

### 5. Performance Optimization

```dart
// lib/main.dart

void main() {
  // Deferred loading for large features
  runApp(const MyApp());
}

// Lazy load heavy pages
GoRoute(
  path: '/analytics',
  builder: (context, state) {
    return FutureBuilder(
      future: () async {
        // Dynamic import (tree-shakeable)
        await deferred_analytics.loadLibrary();
        return true;
      }(),
      builder: (context, snapshot) {
        if (snapshot.hasData) {
          return deferred_analytics.AnalyticsPage();
        }
        return const Center(child: CircularProgressIndicator());
      },
    );
  },
),
```

**Build flags for web:**
```bash
flutter build web \
  --release \
  --web-renderer canvaskit \
  --tree-shake-icons \
  --pwa-strategy offline-first \
  --dart-define=ENV=production
```

### 6. PWA Configuration

```json
// web/manifest.json
{
  "name": "My Flutter App",
  "short_name": "MyApp",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#0f172a",
  "theme_color": "#06b6d4",
  "icons": [
    { "src": "icons/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "icons/icon-512.png", "sizes": "512x512", "type": "image/png" }
  ]
}
```

### 7. Deployment

```yaml
# .github/workflows/web-deploy.yml
name: Deploy Web
on:
  push:
    branches: [main]

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: subosito/flutter-action@v2
      
      - run: flutter build web --release --web-renderer canvaskit
      
      - name: Falcon Quality Gate
        run: |
          cargo install --git https://github.com/viveky259259/falcon
          falcon ai-score . --json > build/web/falcon-score.json
          
      - name: Deploy to Firebase/Vercel/Cloudflare
        run: firebase deploy --only hosting
```

---

## Web-Specific Gotchas

| Issue | Solution |
|---|---|
| CORS errors | Configure server, use proxy in dev |
| Cookie/localStorage | Use `package:shared_preferences` (web-compatible) |
| File uploads | Use `package:file_picker` (web implementation) |
| Keyboard shortcuts | `FocusNode` + `RawKeyboardListener` / `Shortcuts` widget |
| Right-click context menu | `Listener` + custom popup, disable browser menu |
| Print/PDF | `package:pdf` + `package:printing` (web-compatible) |
| Back button | GoRouter handles browser back/forward |
| Text selection | Wrap in `SelectionArea` (Flutter 3.3+) |
