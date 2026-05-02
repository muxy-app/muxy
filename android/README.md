# Muxy Android

Android companion app for Muxy. Mirrors the iOS app under `MuxyMobile/`:
pair with the desktop app, browse projects, view a terminal pane, send
keyboard input. Transport is the desktop app's `MuxyRemoteServer`
WebSocket on port `4865`. Muxy debug builds listen on `4866` (the Connect
screen lets you override the default port when adding a device).

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
device pairing token is stored encrypted with an Android Keystore key
(AES/GCM/NoPadding, 256-bit, generated on first launch). The Keystore
key is hardware-bound and cannot survive a backup-and-restore, so
re-installing from a backup would either leak stale ciphertext or
break authentication. Disabling backup keeps things simple.

**Verifying backup exclusion.** To confirm the credential blob never
leaves the device, after pairing, run:

```
adb shell bmgr backupnow com.muxy.android
adb shell bmgr list transports
```

`bmgr backupnow` reports `Package com.muxy.android with result: Backup is not allowed`
when `allowBackup="false"` is honored. Inspecting the backup transport
output should show no entries for `com.muxy.android`.

**Forget device** (Settings) deletes the encrypted credential blob and
the Android Keystore key. The desktop side keeps the approved-device
record until the user removes it from Mac settings, since there is no
remote-revoke RPC today.

## Connection lifecycle

The app cleanly closes its WebSocket when it goes to the background and
auto-reconnects (silently) when it returns to the foreground or when the
network changes. There is no foreground service in v1, so the device's
battery optimizer can hold up reconnects on aggressive OEMs.

**Aggressive OEMs (Samsung, Xiaomi, OnePlus, Huawei, etc.)** kill
backgrounded apps faster than stock Android, and may delay the
network-callback / foreground reconnect path by tens of seconds. If
reconnect is slow on your phone, whitelist Muxy in
**Settings → Battery → App optimization** (the exact path varies by
manufacturer). This is a known limitation of the no-foreground-service
design and is not fixed in v1.

After a process restart (system kill / reboot), the app reads the last
connected host/port from a small DataStore-backed `LastSession` record
and re-runs the full connect → authenticate → select-project flow on
launch. Pairing credentials are durable across process restarts as long
as the app data and Android Keystore key remain intact.

## Tests + checks

Either run individual Gradle tasks:

```
./gradlew :protocol:test :net:test :terminal:test :app:test
./gradlew detekt ktlintCheck lint
./gradlew :app:assembleDebug
./gradlew :app:assembleRelease   # exercises R8 rules
```

Or run the bundled script (the same set the CI uses):

```
../scripts/checks-android.sh        # detekt + ktlint + lint + tests + APK
../scripts/checks-android.sh --fix  # auto-format with ktlint then run checks
```

`:protocol:test` covers JSON round-trips for every DTO, the three
custom envelope shapes (`MuxyMessage`, `MuxyParams`/`MuxyResult`/
`MuxyEventData`, `SplitNodeDTO`), and the Swift-style enum-with-
associated-values shapes (`PaneOwnerDTO`, `NotificationDTO.SourceDTO`).
`:net:test` drives `MuxyClient` against an OkHttp `MockWebServer` for
authenticate-then-pair, RPC round-trip, fire-and-forget `terminalInput`,
event delivery, silent reconnect, and pending-request cancellation, plus
the diagnostic ring buffer, exponential backoff with jitter, the
DataStore-backed `SavedDevicesStore`, and the `DeviceCredentialsStore`
encryption / forget-device flows (using a fake `CryptoBox`).

## Release builds + signing

Debug builds use the default Android debug keystore. Release builds pick
up a JKS keystore via four environment variables:

```
MUXY_ANDROID_KEYSTORE_PATH        # absolute path to .jks
MUXY_ANDROID_KEYSTORE_PASSWORD
MUXY_ANDROID_KEY_ALIAS
MUXY_ANDROID_KEY_PASSWORD
```

If `MUXY_ANDROID_KEYSTORE_PATH` is unset, `:app:assembleRelease` still
runs (R8 + signing with the debug key) so contributors can validate that
release-mode obfuscation rules cover every kotlinx.serialization
`@Serializable` shape locally. The CI release workflow
(`.github/workflows/release-android.yml`) decodes a base64 keystore
secret into the runner and exports those four variables before
assembling the signed APK on `v*-android` tags. The same workflow
attaches both the APK and the R8 `mapping.txt` to the GitHub release
draft so user-supplied stack traces can be de-obfuscated later.

## Phase status

Tracked in `docs/plans/android-companion.md` at the repo root. v1 is now
feature-complete through Phase 13: scaffolding, protocol port, network
client, pairing + credential vault, UI shell, terminal rendering with
Termux core, accessory bar, workspace + tab picker, full VCS sheet stack
(status / branches / worktrees / create PR), notifications, lifecycle
hooks (foreground / background, network callback, silent reconnect,
process-death recovery via `LastSessionStore`), Settings screen
(font size 8…24, Use Nerd Font, About, Forget Device), error-report
sheet, splash screen, responsive layouts, accessibility passes, CI
(detekt + ktlint + lint + tests + APK), and a tag-driven release
workflow.
