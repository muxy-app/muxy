# Muxy Android

Android companion app for Muxy. Mirrors the iOS app under `MuxyMobile/`:
pair with the desktop app, browse projects, view a terminal pane, send
keyboard input. Transport is the desktop app's `MuxyRemoteServer`
WebSocket on port `4865`.

The Android binary and everything under `android/` is licensed under
**GPL-3.0** because the terminal layer vendors Termux's `terminal-emulator`
and `terminal-view`. The rest of the Muxy repo keeps its existing license.

## Build requirements

| Tool | Version |
|---|---|
| Android Studio | Ladybug or newer |
| JDK | 17 |
| Android SDK platform | 35 (Android 15) |
| Build tools | 35.0.0 |
| Min SDK | 31 (Android 12) |
| Target / compile SDK | 35 |
| Gradle | 8.10.2 (downloaded by the wrapper) |
| Android Gradle Plugin | 8.7.3 |
| Kotlin | 2.0.21 |

## First-time setup

1. Install Android Studio and accept all SDK licenses for API 35.
2. Copy `local.properties.example` to `local.properties` and set
   `sdk.dir` to the path of your Android SDK.
3. From `android/`, run the Gradle wrapper to fetch dependencies and
   build the debug APK:

   ```
   ./gradlew assembleDebug
   ```

The unsigned debug APK lands at
`app/build/outputs/apk/debug/app-debug.apk`.

## Modules

| Module | Type | Purpose |
|---|---|---|
| `:app` | Android application | UI, navigation, ViewModels |
| `:protocol` | JVM library | Wire DTOs, envelope, codec |
| `:net` | Android library | WebSocket client, connection manager, credential store |
| `:terminal` | Android library | Vendored Termux terminal core + Compose wrapper |

## Security notes

**Trusted-network only.** The desktop server speaks plain `ws://` (no
TLS). The pairing token and every keystroke travel in the clear. Use
the app on Tailscale or a VPN you control. Do not pair across an open
Wi-Fi network. The Connect screen surfaces this warning the first time
you add a device.

**Cleartext traffic is enabled app-wide** (`network_security_config.xml`)
because users type arbitrary hosts and IPs at runtime. Android cannot
scope cleartext to a dynamic host list, so the choice is "all hosts"
or "fixed allow-list", and we picked the former for now.

**Auto-backup is disabled** (`android:allowBackup="false"`). The
device pairing token is stored encrypted with an Android Keystore key.
That key cannot survive a backup-and-restore, so re-installing from a
backup would either leak stale ciphertext or break authentication.
Disabling backup keeps things simple.

## Phase status

Tracked in `docs/plans/android-companion.md` at the repo root. This
README covers Phase 1 only: project scaffolding. The protocol port,
networking, pairing, and terminal layers come in later phases.
