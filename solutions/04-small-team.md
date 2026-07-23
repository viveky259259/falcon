# Solution 4: Flutter Architecture for Small Teams (1-2 Developers)

## The Problem

A solo developer or pair needs to build and maintain a Flutter app efficiently. The architecture must be simple enough to move fast, but structured enough to not become unmaintainable as the app grows. No time for over-engineering.

## Architecture: Pragmatic Feature-First

```
┌────────────────────────────────────────────┐
│          Simple Feature-First              │
│                                            │
│  ┌────────────────────────────────────┐   │
│  │          lib/                       │   │
│  │  ┌──────────┐  ┌──────────┐       │   │
│  │  │  core/   │  │features/ │       │   │
│  │  │ (shared) │  │  (pages) │       │   │
│  │  └──────────┘  └──────────┘       │   │
│  │                                    │   │
│  │  State: Riverpod (simple + scalable)  │
│  │  Nav:   GoRouter (URL-ready)          │
│  │  DI:    Riverpod (built-in)           │
│  └────────────────────────────────────┘   │
└────────────────────────────────────────────┘
```

## Project Structure

```
my_app/
├── pubspec.yaml
├── falcon.yaml                    # Keep it simple
├── lib/
│   ├── main.dart                  # Entry point
│   ├── app.dart                   # MaterialApp.router
│   ├── router.dart                # GoRouter config
│   │
│   ├── core/                      # Shared stuff
│   │   ├── theme.dart             # ThemeData
│   │   ├── constants.dart         # App constants
│   │   ├── extensions.dart        # Dart extensions
│   │   ├── network/
│   │   │   ├── api_client.dart    # Dio/http wrapper
│   │   │   └── api_endpoints.dart
│   │   ├── storage/
│   │   │   └── local_storage.dart # SharedPreferences wrapper
│   │   └── widgets/               # Reusable widgets
│   │       ├── app_button.dart
│   │       ├── app_text_field.dart
│   │       └── loading_overlay.dart
│   │
│   ├── features/                  # One folder per screen/feature
│   │   ├── auth/
│   │   │   ├── login_page.dart
│   │   │   ├── register_page.dart
│   │   │   └── auth_provider.dart     # Riverpod provider
│   │   │
│   │   ├── home/
│   │   │   ├── home_page.dart
│   │   │   └── home_provider.dart
│   │   │
│   │   ├── profile/
│   │   │   ├── profile_page.dart
│   │   │   ├── edit_profile_page.dart
│   │   │   └── profile_provider.dart
│   │   │
│   │   └── settings/
│   │       ├── settings_page.dart
│   │       └── settings_provider.dart
│   │
│   └── models/                    # Shared data models
│       ├── user.dart
│       ├── product.dart
│       └── order.dart
│
├── test/
│   ├── features/
│   │   ├── auth/
│   │   │   └── auth_provider_test.dart
│   │   └── home/
│   │       └── home_provider_test.dart
│   └── core/
│       └── api_client_test.dart
│
└── assets/
```

**That's it. No `domain/`, no `data/`, no `repositories/`, no `usecases/`.** Those add value at 10+ developers, not 1-2.

## Key Technical Decisions

### 1. State Management: Riverpod (One Tool for Everything)

```dart
// features/auth/auth_provider.dart

// Simple state
final currentUserProvider = StateProvider<User?>((ref) => null);

// Async data
final userProfileProvider = FutureProvider.family<User, String>((ref, userId) async {
  final api = ref.read(apiClientProvider);
  return api.getUser(userId);
});

// Complex state with notifier
final authProvider = AsyncNotifierProvider<AuthNotifier, AuthState>(AuthNotifier.new);

class AuthNotifier extends AsyncNotifier<AuthState> {
  @override
  Future<AuthState> build() async {
    final token = await ref.read(localStorageProvider).getString('token');
    if (token != null) {
      final user = await ref.read(apiClientProvider).getMe(token);
      return AuthState.authenticated(user);
    }
    return const AuthState.unauthenticated();
  }

  Future<void> login(String email, String password) async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() async {
      final token = await ref.read(apiClientProvider).login(email, password);
      await ref.read(localStorageProvider).setString('token', token);
      final user = await ref.read(apiClientProvider).getMe(token);
      return AuthState.authenticated(user);
    });
  }

  Future<void> logout() async {
    await ref.read(localStorageProvider).remove('token');
    state = const AsyncData(AuthState.unauthenticated());
  }
}
```

### 2. Navigation: Simple GoRouter

```dart
// lib/router.dart

final routerProvider = Provider<GoRouter>((ref) {
  final auth = ref.watch(authProvider);

  return GoRouter(
    initialLocation: '/',
    redirect: (context, state) {
      final isLoggedIn = auth.valueOrNull?.isAuthenticated ?? false;
      final isAuthRoute = state.matchedLocation.startsWith('/auth');

      if (!isLoggedIn && !isAuthRoute) return '/auth/login';
      if (isLoggedIn && isAuthRoute) return '/';
      return null;
    },
    routes: [
      GoRoute(path: '/auth/login', builder: (_, __) => const LoginPage()),
      GoRoute(path: '/auth/register', builder: (_, __) => const RegisterPage()),
      ShellRoute(
        builder: (_, __, child) => AppShell(child: child),
        routes: [
          GoRoute(path: '/', builder: (_, __) => const HomePage()),
          GoRoute(path: '/profile', builder: (_, __) => const ProfilePage()),
          GoRoute(path: '/settings', builder: (_, __) => const SettingsPage()),
        ],
      ),
    ],
  );
});
```

### 3. Network Layer: Thin Wrapper

```dart
// lib/core/network/api_client.dart

final apiClientProvider = Provider<ApiClient>((ref) => ApiClient());

class ApiClient {
  late final Dio _dio;

  ApiClient() {
    _dio = Dio(BaseOptions(
      baseUrl: const String.fromEnvironment('API_URL', defaultValue: 'https://api.example.com'),
      connectTimeout: const Duration(seconds: 10),
    ));

    _dio.interceptors.add(LogInterceptor(requestBody: true, responseBody: true));
  }

  void setToken(String token) {
    _dio.options.headers['Authorization'] = 'Bearer $token';
  }

  Future<T> get<T>(String path, {T Function(dynamic)? fromJson}) async {
    final response = await _dio.get(path);
    return fromJson != null ? fromJson(response.data) : response.data;
  }

  Future<T> post<T>(String path, {dynamic data, T Function(dynamic)? fromJson}) async {
    final response = await _dio.post(path, data: data);
    return fromJson != null ? fromJson(response.data) : response.data;
  }
}
```

### 4. Models: Keep Them Simple

```dart
// lib/models/user.dart

class User {
  final String id;
  final String name;
  final String email;
  final String? avatarUrl;

  const User({required this.id, required this.name, required this.email, this.avatarUrl});

  factory User.fromJson(Map<String, dynamic> json) => User(
    id: json['id'] as String,
    name: json['name'] as String,
    email: json['email'] as String,
    avatarUrl: json['avatar_url'] as String?,
  );

  Map<String, dynamic> toJson() => {
    'id': id, 'name': name, 'email': email, 'avatar_url': avatarUrl,
  };
}
```

**Don't use freezed/json_serializable for < 10 models.** Hand-written fromJson is faster to iterate on when you're moving fast.

### 5. Testing Strategy for Small Teams

```
Focus on: Provider tests (business logic)
Skip for now: Widget tests (change too often early on)
Add later: Integration tests (when stable)
```

```dart
// test/features/auth/auth_provider_test.dart

void main() {
  late ProviderContainer container;

  setUp(() {
    container = ProviderContainer(overrides: [
      apiClientProvider.overrideWithValue(MockApiClient()),
      localStorageProvider.overrideWithValue(MockLocalStorage()),
    ]);
  });

  test('login stores token and fetches user', () async {
    final notifier = container.read(authProvider.notifier);
    await notifier.login('test@example.com', 'password');

    final state = container.read(authProvider);
    expect(state.value?.isAuthenticated, true);
  });
}
```

### 6. Falcon Config for Small Teams

```yaml
# falcon.yaml — minimal, high-value rules only
metrics:
  cyclomatic_complexity: 25     # More lenient
  lines_of_code: 400            # Allow longer files
  number_of_parameters: 7

rules:
  - avoid-empty-catch            # Catches real bugs
  - ensure-dispose-lifecycle     # Prevents memory leaks
  - avoid-unawaited-futures      # Prevents crashes
  - avoid-hardcoded-credentials  # Security
  - avoid-print-in-production    # Use logger

exclude:
  - "**/*.g.dart"
  - "**/*.freezed.dart"
```

```bash
# One command to check everything
falcon x manage health .
```

---

## When to Graduate to Clean Architecture

| Signal | Threshold |
|---|---|
| Team size | 3+ developers |
| Feature count | 10+ features |
| Codebase size | 200+ Dart files |
| API complexity | Multiple backends, GraphQL |
| Test coverage needs | > 60% required |

When these thresholds hit, refactor incrementally:
```bash
falcon x refactor-sim --scenario clean-architecture
# Shows: 45 files affected, 22 hours estimated
```

---

## Checklist for Solo/Pair Dev

- [ ] Feature-first folder structure
- [ ] Riverpod for all state (no mixing providers)
- [ ] GoRouter for navigation (URL-ready from day 1)
- [ ] One ApiClient class (not per-feature)
- [ ] Models in `/models` (shared, not per-feature)
- [ ] `falcon x manage health .` passes 70+
- [ ] Provider tests for business logic
- [ ] CI runs `falcon check --fail-on error`
