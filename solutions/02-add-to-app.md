# Solution 2: Add-to-App — Flutter Embedded in Native Android/iOS

## The Problem

An existing native Android (Kotlin) and iOS (Swift) app needs to add Flutter screens for new features, while keeping existing native screens working. The Flutter module must integrate seamlessly — sharing navigation, authentication, and data with the native host.

## Architecture Overview

```
┌────────────────────────────────────────────────────────┐
│                  Native Host App                        │
│                                                        │
│  ┌──────────────────┐    ┌──────────────────────────┐  │
│  │  Native Screens  │    │   Flutter Module          │  │
│  │  (Kotlin/Swift)  │    │   (embedded engine)       │  │
│  │                  │    │                            │  │
│  │  ┌────────────┐  │    │  ┌──────────────────────┐ │  │
│  │  │ Home (nat) │  │    │  │ Checkout (Flutter)   │ │  │
│  │  │ Profile    │  │    │  │ Product List         │ │  │
│  │  │ Settings   │  │    │  │ Payment Form         │ │  │
│  │  └────────────┘  │    │  └──────────────────────┘ │  │
│  │                  │    │                            │  │
│  └──────┬───────────┘    └────────────┬───────────────┘  │
│         │                             │                  │
│  ┌──────┴─────────────────────────────┴──────────────┐  │
│  │          Platform Channel Bridge                   │  │
│  │  ┌─────────┐  ┌──────────┐  ┌─────────────────┐  │  │
│  │  │  Auth   │  │Navigation│  │ Shared Data     │  │  │
│  │  │ Channel │  │ Channel  │  │ Channel         │  │  │
│  │  └─────────┘  └──────────┘  └─────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

## Project Structure

```
project_root/
├── android_host/                  # Native Android app
│   ├── app/
│   │   ├── src/main/
│   │   │   ├── kotlin/com/example/
│   │   │   │   ├── MainActivity.kt
│   │   │   │   ├── FlutterBridge.kt       # Flutter engine manager
│   │   │   │   ├── channels/
│   │   │   │   │   ├── AuthChannel.kt
│   │   │   │   │   ├── NavigationChannel.kt
│   │   │   │   │   └── DataChannel.kt
│   │   │   │   └── screens/               # Native screens
│   │   │   └── AndroidManifest.xml
│   │   └── build.gradle
│   ├── settings.gradle                    # Includes Flutter module
│   └── build.gradle
│
├── ios_host/                      # Native iOS app
│   ├── Runner/
│   │   ├── AppDelegate.swift
│   │   ├── FlutterBridge.swift            # Flutter engine manager
│   │   ├── Channels/
│   │   │   ├── AuthChannel.swift
│   │   │   ├── NavigationChannel.swift
│   │   │   └── DataChannel.swift
│   │   └── Screens/                       # Native screens
│   ├── Podfile
│   └── Runner.xcodeproj
│
├── flutter_module/                # Flutter module (add-to-app)
│   ├── pubspec.yaml
│   ├── .android/                  # Auto-generated
│   ├── .ios/                      # Auto-generated
│   ├── lib/
│   │   ├── main.dart              # Entry point
│   │   ├── app.dart               # MaterialApp
│   │   ├── channels/              # Platform channel Dart side
│   │   │   ├── auth_channel.dart
│   │   │   ├── nav_channel.dart
│   │   │   └── data_channel.dart
│   │   ├── features/
│   │   │   ├── checkout/
│   │   │   ├── product_list/
│   │   │   └── payment/
│   │   └── shared/
│   │       ├── theme.dart         # Match native app theme
│   │       └── di.dart
│   └── test/
│
└── shared_contracts/              # Pigeon-generated type-safe channels
    ├── pigeon/
    │   ├── auth.dart              # Pigeon definition
    │   ├── navigation.dart
    │   └── data.dart
    └── generate.sh
```

## Key Technical Decisions

### 1. Flutter Engine Management

**Pre-warm the engine for instant screen transitions:**

```kotlin
// android_host: FlutterBridge.kt

class FlutterBridge private constructor(context: Context) {
    
    companion object {
        @Volatile private var instance: FlutterBridge? = null
        
        fun getInstance(context: Context): FlutterBridge {
            return instance ?: synchronized(this) {
                instance ?: FlutterBridge(context.applicationContext).also { instance = it }
            }
        }
    }
    
    val engine: FlutterEngine = FlutterEngine(context).apply {
        // Pre-warm with initial route
        navigationChannel.setInitialRoute("/")
        dartExecutor.executeDartEntrypoint(
            DartExecutor.DartEntrypoint.createDefault()
        )
    }
    
    val engineGroup: FlutterEngineGroup = FlutterEngineGroup(context)
    
    init {
        // Cache the engine for instant display
        FlutterEngineCache.getInstance().put("main_engine", engine)
        
        // Register platform channels
        AuthChannel.register(engine)
        NavigationChannel.register(engine)
        DataChannel.register(engine)
    }
    
    fun createEngineForRoute(route: String): FlutterEngine {
        return engineGroup.createAndRunEngine(
            context,
            DartEntrypoint.createDefault(),
            route
        )
    }
}
```

```swift
// ios_host: FlutterBridge.swift

class FlutterBridge {
    static let shared = FlutterBridge()
    
    let engine: FlutterEngine
    let engineGroup: FlutterEngineGroup
    
    private init() {
        engineGroup = FlutterEngineGroup(name: "super_app", project: nil)
        engine = engineGroup.makeEngine(withEntrypoint: nil, libraryURI: nil)
        engine.run()
        
        // Register channels
        AuthChannel.register(with: engine)
        NavigationChannel.register(with: engine)
        DataChannel.register(with: engine)
        
        // Cache for FlutterViewController
        GeneratedPluginRegistrant.register(with: engine)
    }
    
    func makeViewController(route: String) -> FlutterViewController {
        let newEngine = engineGroup.makeEngine(
            withEntrypoint: nil,
            libraryURI: nil,
            initialRoute: route
        )
        GeneratedPluginRegistrant.register(with: newEngine)
        return FlutterViewController(engine: newEngine, nibName: nil, bundle: nil)
    }
}
```

### 2. Type-Safe Platform Channels with Pigeon

```dart
// shared_contracts/pigeon/auth.dart
import 'package:pigeon/pigeon.dart';

class AuthUser {
  String? userId;
  String? email;
  String? token;
  String? displayName;
}

@HostApi()
abstract class AuthHostApi {
  AuthUser? getCurrentUser();
  String? getAuthToken();
  void logout();
}

@FlutterApi()
abstract class AuthFlutterApi {
  void onAuthStateChanged(AuthUser? user);
  void onTokenRefreshed(String newToken);
}
```

Generate bindings:
```bash
# generate.sh
dart run pigeon \
  --input pigeon/auth.dart \
  --dart_out ../flutter_module/lib/channels/auth_channel.g.dart \
  --kotlin_out ../android_host/app/src/main/kotlin/com/example/channels/AuthChannel.g.kt \
  --swift_out ../ios_host/Runner/Channels/AuthChannel.g.swift
```

### 3. Navigation Bridge

```dart
// flutter_module/lib/channels/nav_channel.dart

class NavigationBridge {
  static const _channel = MethodChannel('com.example/navigation');
  
  /// Navigate from Flutter to a native screen
  static Future<void> navigateToNative(String route, {Map<String, dynamic>? args}) async {
    await _channel.invokeMethod('navigateTo', {
      'route': route,
      'args': args,
    });
  }
  
  /// Close the current Flutter screen and return to native
  static Future<void> closeFlutter({dynamic result}) async {
    await _channel.invokeMethod('close', {'result': result});
  }
  
  /// Listen for native → Flutter navigation requests
  static void setupHandler(Function(String route, Map<String, dynamic>? args) onNavigate) {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'navigateToFlutter') {
        final route = call.arguments['route'] as String;
        final args = call.arguments['args'] as Map<String, dynamic>?;
        onNavigate(route, args);
      }
    });
  }
}
```

```kotlin
// android_host: NavigationChannel.kt

class NavigationChannel(private val activity: Activity) {
    
    companion object {
        fun register(engine: FlutterEngine, activity: Activity) {
            val channel = MethodChannel(engine.dartExecutor.binaryMessenger, "com.example/navigation")
            val handler = NavigationChannel(activity)
            
            channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "navigateTo" -> {
                        val route = call.argument<String>("route")!!
                        val args = call.argument<Map<String, Any>>("args")
                        handler.navigateToNativeScreen(route, args)
                        result.success(null)
                    }
                    "close" -> {
                        val returnResult = call.argument<Any>("result")
                        activity.setResult(Activity.RESULT_OK, Intent().apply {
                            putExtra("result", returnResult?.toString())
                        })
                        activity.finish()
                        result.success(null)
                    }
                }
            }
        }
    }
    
    private fun navigateToNativeScreen(route: String, args: Map<String, Any>?) {
        when (route) {
            "/native/profile" -> {
                activity.startActivity(Intent(activity, ProfileActivity::class.java))
            }
            "/native/settings" -> {
                activity.startActivity(Intent(activity, SettingsActivity::class.java))
            }
        }
    }
}
```

### 4. Shared Theme Synchronization

```dart
// flutter_module/lib/shared/theme.dart

class NativeThemeBridge {
  static const _channel = MethodChannel('com.example/theme');
  
  static Future<ThemeData> getNativeTheme() async {
    final themeMap = await _channel.invokeMethod<Map>('getTheme');
    
    return ThemeData(
      colorScheme: ColorScheme(
        primary: Color(themeMap!['primaryColor'] as int),
        secondary: Color(themeMap['secondaryColor'] as int),
        surface: Color(themeMap['surfaceColor'] as int),
        // ... map all colors
        brightness: themeMap['isDark'] == true ? Brightness.dark : Brightness.light,
      ),
      fontFamily: themeMap['fontFamily'] as String?,
    );
  }
}
```

### 5. Data Sharing Between Native and Flutter

```dart
// flutter_module/lib/channels/data_channel.dart

class SharedDataBridge {
  static const _channel = MethodChannel('com.example/data');
  
  /// Get user cart from native (SQLite/Room)
  static Future<List<CartItem>> getCart() async {
    final data = await _channel.invokeMethod<List>('getCart');
    return data?.map((e) => CartItem.fromMap(e)).toList() ?? [];
  }
  
  /// Push Flutter changes back to native
  static Future<void> updateCart(List<CartItem> items) async {
    await _channel.invokeMethod('updateCart', {
      'items': items.map((e) => e.toMap()).toList(),
    });
  }
  
  /// Stream native data changes to Flutter
  static Stream<List<CartItem>> watchCart() {
    const eventChannel = EventChannel('com.example/data/cart_stream');
    return eventChannel.receiveBroadcastStream().map((data) {
      return (data as List).map((e) => CartItem.fromMap(e)).toList();
    });
  }
}
```

### 6. Build Configuration

```groovy
// android_host/settings.gradle
setBinding(new Binding([gradle: this]))
evaluate(new File(
    settingsDir,
    '../flutter_module/.android/include_flutter.groovy'
))
include ':flutter_module'
project(':flutter_module').projectDir = new File('../flutter_module')

// android_host/app/build.gradle
dependencies {
    implementation project(':flutter')
}
```

```ruby
# ios_host/Podfile
flutter_application_path = '../flutter_module'
load File.join(flutter_application_path, '.ios', 'Flutter', 'podhelper.rb')

target 'Runner' do
  install_all_flutter_pods(flutter_application_path)
end
```

### 7. Falcon Quality Gates for Add-to-App

```bash
# CI: Analyze Flutter module
falcon analyze flutter_module/ --fail-on error
falcon x check-platform .  # Analyze Kotlin/Swift channel code
falcon ai-score flutter_module/

# Check channel consistency
falcon x manage deps flutter_module/
```

---

## Performance Considerations

| Concern | Solution |
|---|---|
| Cold start (engine init) | Pre-warm engine in Application.onCreate / AppDelegate |
| Memory (multiple engines) | Use FlutterEngineGroup — shares GPU context |
| Screen transitions | Cache engines, use FlutterFragment/FlutterViewController |
| Bundle size | Flutter adds ~5MB to APK — use split APKs |
| Hot reload during dev | Run Flutter module standalone for development |

---

## Migration Strategy (Native → Flutter)

```
Phase 1: Add Flutter infrastructure (engine, channels)
Phase 2: Build ONE new screen in Flutter (lowest risk)
Phase 3: Build all NEW screens in Flutter
Phase 4: Migrate existing screens one at a time (highest traffic last)
Phase 5: Replace native navigation with Flutter navigation
```
