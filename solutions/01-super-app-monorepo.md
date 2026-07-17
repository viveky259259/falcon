# Solution 1: Super App in Flutter — Split Deployed Packages with Monolith Repo

## The Problem

A large organization (20-100+ developers) needs to build a Super App — a single consumer-facing app containing multiple product verticals (payments, shopping, messaging, rides, etc.) — using Flutter. Teams must work independently without blocking each other, yet ship a single unified APK/IPA.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    MONOREPO (Git)                        │
│                                                         │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐   │
│  │ shell/  │  │payments/│  │shopping/│  │ rides/  │   │
│  │ (host)  │  │ module  │  │ module  │  │ module  │   │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘   │
│       │            │            │            │         │
│  ┌────┴────────────┴────────────┴────────────┴────┐    │
│  │              shared/ (core packages)            │    │
│  │  ┌──────┐ ┌────────┐ ┌───────┐ ┌───────────┐  │    │
│  │  │design│ │  auth  │ │network│ │ analytics │  │    │
│  │  │system│ │  core  │ │ layer │ │  engine   │  │    │
│  │  └──────┘ └────────┘ └───────┘ └───────────┘  │    │
│  └────────────────────────────────────────────────┘    │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │              tools/ (build, CI, codegen)         │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Repository Structure

```
super_app/
├── melos.yaml                    # Monorepo orchestration
├── pubspec.yaml                  # Root (workspace)
├── analysis_options.yaml         # Shared lint rules
├── falcon.yaml                   # Falcon config (shared)
│
├── apps/
│   └── shell/                    # Host app (thin shell)
│       ├── pubspec.yaml
│       ├── lib/
│       │   ├── main.dart
│       │   ├── app.dart          # MaterialApp + routing
│       │   ├── di/               # Dependency injection root
│       │   └── navigation/       # Top-level router
│       ├── android/
│       ├── ios/
│       └── test/
│
├── modules/                      # Feature modules (independently deployable)
│   ├── payments/
│   │   ├── pubspec.yaml          # Own dependencies
│   │   ├── lib/
│   │   │   ├── payments.dart     # Public API (barrel file)
│   │   │   ├── src/
│   │   │   │   ├── domain/       # Business logic
│   │   │   │   ├── data/         # Repositories, APIs
│   │   │   │   └── presentation/ # UI
│   │   │   └── navigation/       # Module routes
│   │   └── test/
│   │
│   ├── shopping/
│   │   └── ... (same structure)
│   │
│   ├── messaging/
│   │   └── ...
│   │
│   └── rides/
│       └── ...
│
├── packages/                     # Shared packages
│   ├── design_system/            # Widgets, themes, tokens
│   │   ├── pubspec.yaml
│   │   ├── lib/
│   │   │   ├── atoms/            # Buttons, inputs, icons
│   │   │   ├── molecules/        # Cards, list tiles
│   │   │   ├── organisms/        # App bars, nav bars
│   │   │   ├── tokens/           # Colors, typography, spacing
│   │   │   └── theme/            # ThemeData
│   │   └── test/
│   │
│   ├── auth_core/                # Authentication (shared)
│   ├── network/                  # HTTP client, interceptors
│   ├── analytics/                # Event tracking
│   ├── storage/                  # Local storage abstraction
│   ├── l10n/                     # Localization
│   └── core_utils/               # Extensions, helpers
│
├── tools/
│   ├── ci/                       # CI scripts
│   ├── codegen/                  # Code generation configs
│   └── scripts/                  # Build, release scripts
│
└── docs/
    ├── architecture.md
    ├── module-guide.md
    └── onboarding.md
```

## Key Technical Decisions

### 1. Monorepo Tooling: Melos

```yaml
# melos.yaml
name: super_app
packages:
  - apps/**
  - modules/**
  - packages/**

command:
  bootstrap:
    usePubspecOverrides: true

scripts:
  analyze:
    run: melos exec -- falcon analyze . --fail-on error
    description: Run Falcon on all packages
    
  test:
    run: melos exec -- flutter test --coverage
    description: Run tests in all packages
    
  build:shell:
    run: cd apps/shell && flutter build apk --release
    description: Build the shell app
    
  gen:
    run: melos exec -- dart run build_runner build --delete-conflicting-outputs
    packageFilters:
      dependsOn: build_runner
```

### 2. Module Interface Contract

Every module exposes exactly ONE public API file:

```dart
// modules/payments/lib/payments.dart (barrel file)

/// Public API for the Payments module.
/// Teams depend on this file ONLY — never import from src/.
library payments;

// Navigation
export 'src/navigation/payments_routes.dart';

// Public models (shared across modules)
export 'src/domain/models/payment_method.dart';
export 'src/domain/models/transaction.dart';

// Module initializer
export 'src/payments_module.dart';
```

```dart
// modules/payments/lib/src/payments_module.dart

class PaymentsModule {
  /// Register this module's dependencies and routes.
  static void register(GetIt di, GoRouter router) {
    // DI
    di.registerLazySingleton<PaymentRepository>(
      () => PaymentRepositoryImpl(di<NetworkClient>()),
    );
    
    // Routes
    router.addRoutes(PaymentsRoutes.routes);
  }
  
  /// Deferred initialization (called after app starts).
  static Future<void> init() async {
    // Preload payment methods, etc.
  }
}
```

### 3. Navigation: Federated GoRouter

```dart
// apps/shell/lib/navigation/app_router.dart

final appRouter = GoRouter(
  initialLocation: '/',
  routes: [
    ShellRoutes.routes,       // Shell routes (home, settings)
    PaymentsRoutes.routes,     // /payments/*
    ShoppingRoutes.routes,     // /shopping/*
    MessagingRoutes.routes,    // /messaging/*
    RidesRoutes.routes,        // /rides/*
  ],
);
```

Each module defines its own routes:

```dart
// modules/payments/lib/src/navigation/payments_routes.dart

class PaymentsRoutes {
  static final routes = GoRoute(
    path: '/payments',
    builder: (context, state) => const PaymentsHomePage(),
    routes: [
      GoRoute(
        path: 'history',
        builder: (context, state) => const PaymentHistoryPage(),
      ),
      GoRoute(
        path: 'add-method',
        builder: (context, state) => const AddPaymentMethodPage(),
      ),
    ],
  );
}
```

### 4. Dependency Injection: Layered GetIt

```dart
// apps/shell/lib/di/injection.dart

Future<void> configureDependencies() async {
  final di = GetIt.instance;
  
  // Layer 1: Core packages (network, storage, analytics)
  di.registerLazySingleton<NetworkClient>(() => DioNetworkClient());
  di.registerLazySingleton<StorageService>(() => HiveStorageService());
  di.registerLazySingleton<AnalyticsEngine>(() => FirebaseAnalytics());
  
  // Layer 2: Auth (depends on core)
  AuthModule.register(di);
  
  // Layer 3: Feature modules (depend on core + auth)
  PaymentsModule.register(di, appRouter);
  ShoppingModule.register(di, appRouter);
  MessagingModule.register(di, appRouter);
  RidesModule.register(di, appRouter);
}
```

### 5. Inter-Module Communication

Modules NEVER import each other directly. Communication via:

**Option A: Event Bus (loose coupling)**
```dart
// packages/core_utils/lib/event_bus.dart
class AppEventBus {
  static final _controller = StreamController<AppEvent>.broadcast();
  static Stream<AppEvent> get stream => _controller.stream;
  static void fire(AppEvent event) => _controller.add(event);
}

// In payments module:
AppEventBus.fire(PaymentCompletedEvent(transactionId: '123'));

// In messaging module (listening):
AppEventBus.stream
  .whereType<PaymentCompletedEvent>()
  .listen((event) => showPaymentReceipt(event.transactionId));
```

**Option B: Service Locator (typed contracts)**
```dart
// packages/core_utils/lib/contracts/payment_contract.dart
abstract class PaymentContract {
  Future<bool> processPayment(double amount, String currency);
  Stream<PaymentStatus> watchStatus(String transactionId);
}

// Payments module registers implementation:
di.registerLazySingleton<PaymentContract>(() => PaymentService());

// Shopping module uses the contract:
final payment = di<PaymentContract>();
await payment.processPayment(99.99, 'USD');
```

### 6. Build & Deploy Pipeline

```yaml
# .github/workflows/super-app.yml
name: Super App CI

on:
  pull_request:
  push:
    branches: [main]

jobs:
  detect-changes:
    runs-on: ubuntu-latest
    outputs:
      modules: ${{ steps.changes.outputs.modules }}
    steps:
      - uses: actions/checkout@v4
      - id: changes
        run: |
          CHANGED=$(git diff --name-only origin/main | grep -oP 'modules/\K[^/]+' | sort -u | jq -R . | jq -s .)
          echo "modules=$CHANGED" >> $GITHUB_OUTPUT

  test-changed:
    needs: detect-changes
    runs-on: ubuntu-latest
    strategy:
      matrix:
        module: ${{ fromJson(needs.detect-changes.outputs.modules) }}
    steps:
      - uses: actions/checkout@v4
      - uses: subosito/flutter-action@v2
      - run: |
          cd modules/${{ matrix.module }}
          flutter test --coverage
          falcon analyze . --fail-on error

  test-shared:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: subosito/flutter-action@v2
      - run: |
          melos bootstrap
          melos run test -- --select-scope packages/
          falcon manage health apps/shell

  build-shell:
    needs: [test-changed, test-shared]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: subosito/flutter-action@v2
      - run: |
          melos bootstrap
          cd apps/shell
          flutter build apk --release
          falcon ai-score . --json > score.json
```

### 7. Team Ownership Model

```
Team Payments    → modules/payments/ + owns PaymentContract
Team Shopping    → modules/shopping/
Team Messaging   → modules/messaging/
Team Rides       → modules/rides/
Team Platform    → packages/* + apps/shell/ + tools/
```

**Rules enforced by Falcon:**
```bash
# Each team runs Falcon on their module
falcon analyze modules/payments/ --fail-on error
falcon manage arch modules/payments/  # No cross-module imports
falcon x check-layers modules/payments/ # Clean Architecture enforced
falcon x drift modules/payments/ --since main  # Convention adherence
```

### 8. Performance: Deferred Loading

```dart
// apps/shell/lib/navigation/app_router.dart
import 'package:payments/payments.dart' deferred as payments;
import 'package:shopping/shopping.dart' deferred as shopping;

GoRoute(
  path: '/payments',
  builder: (context, state) {
    return FutureBuilder(
      future: payments.loadLibrary(),
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.done) {
          return payments.PaymentsHomePage();
        }
        return const ModuleLoadingScreen();
      },
    );
  },
),
```

### 9. Falcon Integration for Super App

```yaml
# falcon.yaml (root)
metrics:
  cyclomatic_complexity: 15
  lines_of_code: 200
  maximum_nesting_level: 4

rules:
  - avoid-dynamic
  - ensure-dispose-lifecycle
  - avoid-empty-catch
  - avoid-unawaited-futures

# Super App specific
exclude:
  - "**/*.g.dart"
  - "**/*.freezed.dart"
  - "**/generated/**"
```

```bash
# CI: Per-module quality gates
for module in modules/*/; do
  echo "=== Analyzing $module ==="
  falcon ai-score "$module"
  falcon manage deps "$module"
  falcon x check-layers "$module"
done

# Full app health
falcon manage health apps/shell
falcon manage all apps/shell
```

---

## Anti-Patterns to Avoid

| Anti-Pattern | Why It's Bad | Solution |
|---|---|---|
| Module A imports Module B's `src/` | Breaks encapsulation, creates hidden coupling | Only import barrel files |
| Shared mutable state | Race conditions, unpredictable behavior | Use Riverpod/BLoC for state isolation |
| God packages (one package with everything) | Defeats the purpose of modularity | Split by domain, max 50 files per package |
| Direct database access from UI | Violates Clean Architecture | Repository pattern + DI |
| Skipping module boundaries for "quick fix" | Tech debt snowball | Falcon `x check-layers` in CI blocks violations |

---

## Scaling Checklist

- [ ] Melos workspace configured with all packages
- [ ] Each module has a barrel file with public API only
- [ ] GoRouter routes are federated per module
- [ ] GetIt DI is layered (core → auth → features)
- [ ] Inter-module communication via EventBus or contracts
- [ ] CI runs module-specific tests (only changed modules)
- [ ] Falcon `x check-layers` enforced in CI
- [ ] Deferred loading for large modules
- [ ] Code ownership (CODEOWNERS file) per module
- [ ] Module creation template/generator script
