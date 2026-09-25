# Mobile SDK

How to build the Muxy phone apps on `muxy-mobile`, the Rust client library
with generated Swift and Kotlin bindings. This guide explains behavior. For
exact signatures, read the generated `muxy_mobile.swift` or `muxy_mobile.kt`.
The Rust source is in `crates/muxy-mobile`, and
`crates/muxy-mobile/tests/sdk.rs` runs the whole flow against a real server.

## How it works

The app embeds the SDK, which connects directly to `muxy-server` on the user's
computer over TLS 1.3. There is no cloud service: the phone must reach the
computer on the same network or through a VPN such as Tailscale.

1. The computer shows a QR code holding a pairing link.
2. The app pairs with that link once and stores the returned
   `ServerCredential`.
3. From then on the app connects with the credential.

The server owns projects and terminal sessions, the same ones the desktop app
and the terminal client show. The SDK keeps each attached terminal's screen up
to date and acknowledges screen updates to the server. The app draws the
screen and sends input.

## Rules

- Make every call that talks to the server off the main thread. Use one
  serial queue per connection so keystrokes stay in order.
- Keep `ConnectionListener.onEvent` short. Hop to the main thread, and never
  call the server from inside it.
- Events carry ids, not data. Each one means "read this again";
  `filesChanged` also says which paths changed.
- The SDK never reconnects by itself. After `disconnected`, the connection and
  every `Terminal` and `Scrollback` from it are finished.
- Store `ServerCredential` in the Keychain or behind an Android Keystore key.
  Never log pairing links or credentials.
- Never work around `IdentityMismatch`. The user must pair again.
- Call `resize` and `endSession` only when the user asks. Both affect every
  client showing the session.
- Don't edit the generated bindings. Change the Rust SDK in
  `crates/muxy-mobile`, then rebuild it.
- During the beta, the SDK and the server must have the same compatibility
  identifier; see [Getting the SDK](#getting-the-sdk). A mismatch fails with
  `IncompatibleVersion`.

## Suggested order

1. Get the SDK and link it into the app.
2. Pairing: scan or paste a link, confirm the computer, name the device.
3. Credential storage, and a list of paired computers.
4. Connecting, event handling, and reconnecting.
5. Projects and their sessions.
6. The terminal view: drawing the screen, the keyboard, accessory keys, and
   paste.
7. Scrollback, then taps and scrolling for programs that use the mouse.
8. Agent activity.
9. Git and files.
10. Error states for every case in [Errors](#errors).

## Getting the SDK

Every beta release publishes the SDK next to the desktop app, on the
[releases page](https://github.com/muxy-app/muxy/releases):

| File | Contents |
| --- | --- |
| `muxy-mobile-<version>-ios.zip` | `MuxyMobile.xcframework` and `muxy_mobile.swift` |
| `muxy-mobile-<version>-android.zip` | `jniLibs/` and `muxy_mobile.kt` |
| `muxy-mobile-<version>.json` | `version`, `compatibility`, and the `sha256` of each zip |

Pin one version in the app repository, together with the SHA-256 that its JSON
lists for the zip. A small script downloads the zip, checks it, and unpacks it
for [Project setup](#project-setup):

```sh
set -euo pipefail
VERSION=2.0.0-beta-1234
SHA256=<the ios zip's sha256 from muxy-mobile-2.0.0-beta-1234.json>
ZIP=muxy-mobile-$VERSION-ios.zip
curl -fsSLO "https://github.com/muxy-app/muxy/releases/download/v$VERSION/$ZIP"
echo "$SHA256  $ZIP" | shasum -a 256 -c
unzip -oq "$ZIP" -d MuxyMobile
```

- On iOS, the zip also works as a Swift Package Manager binary target. Name
  the target `MuxyMobile` to match the XCFramework, use the zip's URL and
  SHA-256, and add `muxy_mobile.swift` to a target that depends on it.
- The phone connects only to a Muxy build with the same `compatibility`.
  `muxy --build-info` prints it for the computer's build. When a Muxy update
  changes it, move the pin to that release.

## Build the SDK

The SDK builds from the Muxy repository on a Mac with Xcode and `rustup`. The
script installs the Rust targets it needs. It builds Android libraries only
when [cargo-ndk](https://github.com/bbqsrc/cargo-ndk) and the Android NDK are
installed.

```sh
scripts/build-mobile-sdk.sh              # writes target/mobile-sdk
scripts/build-mobile-sdk.sh ~/muxy-sdk   # or another folder

# For Android too
cargo install cargo-ndk --locked --version 4.1.2
export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/<version>
scripts/build-mobile-sdk.sh

# Package a full build into the release files, as CI does
python3 scripts/package-mobile-sdk.py 2.0.0-beta-1234 target/mobile-sdk ~/muxy-sdk-release
```

```text
target/mobile-sdk/
├── MuxyMobile.xcframework        static library for iPhone and the Simulator
├── swift/
│   ├── muxy_mobile.swift         the Swift API
│   ├── muxy_mobileFFI.h          already inside the XCFramework
│   └── muxy_mobileFFI.modulemap  already inside the XCFramework
├── ios/                          intermediate files; ignore
└── android/                      only when cargo-ndk is installed
    ├── jniLibs/<abi>/libmuxy_mobile.so   arm64-v8a, armeabi-v7a, x86_64
    └── kotlin/uniffi/muxy_mobile/muxy_mobile.kt
```

## Project setup

### iOS

1. Add `MuxyMobile.xcframework` to the app target and set it to **Do Not
   Embed**. It is a static library.
2. Add `muxy_mobile.swift` to the app target; a local build has it in
   `swift/`. It imports the `muxy_mobileFFI` module that the XCFramework
   provides.
3. Add these Info.plist keys:

```xml
<!-- Asked the first time the app connects to a local network address -->
<key>NSLocalNetworkUsageDescription</key>
<string>Muxy connects to your computer on this network.</string>

<!-- Only if the app scans the pairing code itself -->
<key>NSCameraUsageDescription</key>
<string>Scan the pairing code that Muxy shows on your computer.</string>

<!-- Lets the Camera app open pairing links in the app -->
<key>CFBundleURLTypes</key>
<array>
  <dict>
    <key>CFBundleURLSchemes</key>
    <array><string>muxy</string></array>
  </dict>
</array>
```

If the user declines local network access, LAN addresses fail with
`Unreachable`. Addresses reached through a VPN still work. No App Transport
Security exception is needed, because the SDK doesn't use HTTP.

### Android

1. Copy `jniLibs/*` into `app/src/main/jniLibs/`; a local build has them in
   `android/jniLibs/`.
2. Copy `muxy_mobile.kt` into the Kotlin sources. Its package is
   `uniffi.muxy_mobile`.
3. Add JNA, which the bindings use to load the library:
   `implementation("net.java.dev.jna:jna:5.17.0@aar")`.
4. Add the manifest entries and R8 rules below.

```xml
<uses-permission android:name="android.permission.INTERNET" />

<activity android:name=".MainActivity" android:exported="true">
  <intent-filter>
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="muxy" android:host="pair" />
  </intent-filter>
</activity>
```

```text
-dontwarn java.awt.*
-keep class com.sun.jna.* { *; }
-keepclassmembers class * extends com.sun.jna.* { public *; }
-keep class uniffi.muxy_mobile.** { *; }
```

If the target SDK enforces Android's local network permission, request it
before the first connection to a LAN address.

## API

Swift names are shown. Kotlin uses the same names without argument labels.

| Call | Returns | Talks to the server |
| --- | --- | --- |
| `parsePairingLink(link:)` | `PairingLink` | no |
| `pair(link:deviceName:)` | `ServerCredential` | yes |
| `Connection.connect(credential:listener:)` | `Connection` | yes |
| `connection.serverVersion()` | `String` | no |
| `connection.projects()` | `[Project]` | yes |
| `connection.sessions(projectId:)` | `[Session]` | yes |
| `connection.createSession(projectId:columns:rows:)` | `Session` | yes |
| `connection.endSession(sessionId:)` | — | yes |
| `connection.attach(sessionId:columns:rows:)` | `Terminal` | yes |
| `connection.activity()` | `Activity` | yes |
| `connection.acknowledgeActivity(eventIds:)` | — | yes |
| `connection.git(projectId:action:)` | `GitReply` | yes |
| `connection.files(projectId:action:)` | `FilesReply` | yes |
| `connection.disconnect()` | — | no |
| `terminal.sessionId()` | `UInt64` | no |
| `terminal.screen()` | `Screen` | no |
| `terminal.sendInput(bytes:)` | — | yes |
| `terminal.sendKey(key:modifiers:)` | — | yes |
| `terminal.paste(text:)` | — | yes |
| `terminal.click(button:row:column:modifiers:)` | — | yes |
| `terminal.scroll(direction:row:column:)` | — | yes |
| `terminal.resize(columns:rows:)` | — | yes |
| `terminal.scrollback(maxRows:)` | `Scrollback` | yes |
| `terminal.detach()` | — | yes |
| `scrollback.lines()` | `[Line]` | no |
| `scrollback.historyRows()` | `UInt64` | no |
| `scrollback.loadOlder(maxRows:)` | `[Line]` | yes |

Calls that don't talk to the server read local state and are fine on the main
thread. Every call that talks to the server can throw.

| Type | Fields or cases |
| --- | --- |
| `ServerCredential` | `serverId`, `serverName`, `hosts`, `port`, `fingerprint` (32 bytes), `deviceId`, `token` (32 bytes) |
| `PairingLink` | `hosts`, `port` |
| `Project` | `id`, `name`, `directory`, `color`, `icon?`, `logo?`, `parentId?`, `isHome`, `isWorktree` |
| `Session` | `id`, `projectId`, `directory`, `status`, `owner?`, `attached` |
| `SessionStatus` | `starting`, `live`, `ended`, `unavailable` |
| `ClientKind` | `desktop`, `tui`, `cli`, `mobile` |
| `Screen` | `columns`, `rows`, `lines`, `cursor`, `title`, `directory`, `historyRows`, `applicationCursorKeys`, `bracketedPaste`, `mouseTracking`, `alternateScroll` |
| `Line` | `spans` |
| `Span` | `text`, `width` (terminal cells), `style` |
| `Style` | `foreground`, `background`, `bold`, `italic`, `faint`, `underline`, `underlineColor`, `strikethrough`, `overline`, `inverse`, `invisible` |
| `TerminalColor` | `default`, `indexed(index)`, `rgb(red, green, blue)` |
| `Underline` | `none`, `single`, `double`, `curly`, `dotted`, `dashed` |
| `Cursor` | `row`, `column`, `visible`, `shape` |
| `CursorShape` | `block`, `bar`, `underline`, `hollow` |
| `Key` | `character(text)`, `enter`, `tab`, `backTab`, `escape`, `backspace`, `insert`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageUp`, `pageDown`, `function(number)` |
| `Modifiers` | `shift`, `alt`, `control` |
| `MouseButton` | `left`, `middle`, `right` |
| `ScrollDirection` | `up`, `down` |
| `Activity` | `agents`, `events` |
| `Agent` | `sessionId`, `projectId`, `provider`, `state` |
| `AgentState` | `unknown`, `idle`, `working`, `blocked` |
| `ActivityEvent` | `id`, `sessionId`, `projectId`, `provider`, `timestamp` (Unix seconds), `kind` |
| `ActivityKind` | `attention`, `completed` |
| `GitAction`, `GitReply`, `FilesAction`, `FilesReply` | see [Git and files](#git-and-files) |
| `ConnectionEvent` | see [Events](#events) |
| `MobileError` | see [Errors](#errors) |

Kotlin differences:

- `UInt16`, `UInt64`, and `UInt8` are `UShort`, `ULong`, and `UByte`. `Data`
  is `ByteArray`.
- Enums whose cases carry no data are enum classes with upper-case
  constants, such as `SessionStatus.LIVE`.
- Enums with data are sealed classes, such as `Key.Character("c")`,
  `ConnectionEvent.ScreenChanged(sessionId)`, and `TerminalColor.Rgb(...)`.
  Their cases without data are objects, such as `Key.Enter` and
  `TerminalColor.Default`.
- `MobileError` is `MobileException`, with one subclass per case.
- Records are data classes without default values, so pass every field.
- `Connection`, `Terminal`, and `Scrollback` are `AutoCloseable`. Close them
  when you're done, or they're freed when garbage-collected. In Swift they're
  freed with their last reference. Releasing a `Connection` disconnects it.

## Threads

A serial queue per connection keeps calls in order and off the main thread.

```swift
/// Runs blocking SDK calls one at a time, off the main thread.
enum SDK {
    private static let queue = DispatchQueue(label: "app.muxy.sdk")

    static func run<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { continuation.resume(with: Result { try work() }) }
        }
    }
}

let projects = try await SDK.run { try connection.projects() }
```

```kotlin
// Runs blocking SDK calls one at a time, off the main thread.
val sdk = Executors.newSingleThreadExecutor().asCoroutineDispatcher()

val projects = withContext(sdk) { connection.projects() }
```

## Pairing

In the desktop app, the user turns on **Allow mobile devices** in Settings →
Mobile and clicks **Show Pairing Code**. On a computer without the desktop
app, `muxy mobile pair` prints the code in the terminal. Either way, the QR
code holds a link:

```text
muxy://pair?v=1&h=192.168.1.20&h=100.101.7.12&h=studio.local&p=7419&f=<64 hex>&s=<32 hex>
```

- `h`: up to 8 addresses, tried in order.
- `p`: the TCP port, 7419 by default.
- `f`: the SHA-256 of the server's certificate. The SDK accepts only that
  certificate.
- `s`: a one-time secret. It works once, expires after 5 minutes, and a newer
  code replaces it.

Get the link from an in-app scanner (VisionKit's `DataScannerViewController`,
or Google's code scanner or ML Kit) or from the `muxy://pair` deep link.
Support pasting a link too; it helps in simulators.

```swift
let target = try parsePairingLink(link: link)   // no network; throws InvalidLink
// Ask the user to confirm, showing target.hosts.first and target.port.
let credential = try await SDK.run { try pair(link: link, deviceName: name) }
try credentialStore.save(credential)
```

```kotlin
val target = parsePairingLink(link)             // no network; throws InvalidLink
// Ask the user to confirm, showing target.hosts.first() and target.port.
val credential = withContext(sdk) { pair(link, deviceName) }
credentialStore.save(credential)
```

- The device name is listed under **Paired devices** on the computer, so let
  the user edit it. On iOS 16 and later, `UIDevice.current.name` returns only
  the model name. On Android, read `Settings.Global.DEVICE_NAME` and fall back
  to `Build.MODEL`. The SDK removes control characters and trims the name to
  64 bytes. An empty name becomes "Phone".
- While pairing, `Unauthorized` means the code expired, was used, or was
  replaced. Ask the user to show a new code.
- Pairing again adds another device on the server. The old entry stays until
  the user revokes it.
- Pair each computer separately, and keep one credential per `serverId`.

## Credentials

`ServerCredential` holds everything needed to reconnect, including a token
that grants shell access to the computer. Keep every field.

- `serverId` is stable; use it as the storage key.
- `serverName` is the computer's name, for display.
- `deviceId` is this phone's id, as `muxy mobile` lists it.
- The addresses don't update. If the computer's address changes and no other
  address works, the user must pair again.

The bindings don't make the record serializable, so mirror it:

```swift
struct StoredCredential: Codable {
    var serverId: String
    var serverName: String
    var hosts: [String]
    var port: UInt16
    var fingerprint: Data
    var deviceId: String
    var token: Data

    init(_ credential: ServerCredential) {
        serverId = credential.serverId
        serverName = credential.serverName
        hosts = credential.hosts
        port = credential.port
        fingerprint = credential.fingerprint
        deviceId = credential.deviceId
        token = credential.token
    }

    var credential: ServerCredential {
        ServerCredential(serverId: serverId, serverName: serverName, hosts: hosts, port: port,
                         fingerprint: fingerprint, deviceId: deviceId, token: token)
    }
}
```

```kotlin
fun ServerCredential.toJson(): String = JSONObject()
    .put("serverId", serverId)
    .put("serverName", serverName)
    .put("hosts", JSONArray(hosts))
    .put("port", port.toInt())
    .put("fingerprint", Base64.encodeToString(fingerprint, Base64.NO_WRAP))
    .put("deviceId", deviceId)
    .put("token", Base64.encodeToString(token, Base64.NO_WRAP))
    .toString()

fun credentialFromJson(text: String): ServerCredential {
    val json = JSONObject(text)
    val hosts = json.getJSONArray("hosts")
    return ServerCredential(
        serverId = json.getString("serverId"),
        serverName = json.getString("serverName"),
        hosts = List(hosts.length()) { hosts.getString(it) },
        port = json.getInt("port").toUShort(),
        fingerprint = Base64.decode(json.getString("fingerprint"), Base64.NO_WRAP),
        deviceId = json.getString("deviceId"),
        token = Base64.decode(json.getString("token"), Base64.NO_WRAP),
    )
}
```

On iOS, store the JSON as a generic-password Keychain item with
`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`. On Android, encrypt it with
an Android Keystore key and exclude it from backups.

## Connecting

`Connection.connect` tries each address for up to 4 seconds. It checks the
certificate against the pinned fingerprint, then signs in with the token. Keep
one connection per computer, and keep a strong reference to it.

```swift
final class ConnectionEvents: ConnectionListener {
    private let handler: @Sendable (ConnectionEvent) -> Void

    init(_ handler: @escaping @Sendable (ConnectionEvent) -> Void) {
        self.handler = handler
    }

    func onEvent(event: ConnectionEvent) {
        handler(event)   // on an SDK thread
    }
}

let events = ConnectionEvents { [weak model] event in
    Task { @MainActor in model?.handle(event) }
}
let connection = try await SDK.run {
    try Connection.connect(credential: credential, listener: events)
}
```

```kotlin
class ConnectionEvents(
    private val handler: (ConnectionEvent) -> Unit,
) : ConnectionListener {
    override fun onEvent(event: ConnectionEvent) = handler(event)   // on an SDK thread
}

val events = ConnectionEvents { event ->
    scope.launch(Dispatchers.Main) { model.handle(event) }
}
val connection = withContext(sdk) { Connection.connect(credential, events) }
```

### Events

| Event | What happened | What to do |
| --- | --- | --- |
| `screenChanged(sessionId)` | New output or cursor movement | Redraw from `screen()` on the next frame |
| `metadataChanged(sessionId)` | Title, folder, history size, or mouse modes changed | Read `screen()` again |
| `sessionEnded(sessionId)` | The session ended, from any client | Show it as ended and drop its `Terminal` |
| `catalogChanged` | Projects were added, changed, or removed | Read `projects()` again |
| `sessionsChanged` | Sessions started or ended, or clients attached or detached | Read the visible session lists again |
| `activityChanged` | Agent states or notifications changed | Read `activity()` again |
| `gitChanged(projectId)` | The watched project's repository changed | Run the Git actions you show again |
| `filesChanged(projectId, paths)` | Files changed in a watched project | Read `paths` and their folders again, or everything when `paths` is empty |
| `serverRestarting` | The server is restarting, usually for an update | Show "Reconnecting…" and connect again shortly |
| `disconnected` | The connection closed. This is always the last event | Drop the connection and its terminals, then reconnect when it makes sense |

## Projects and sessions

A project is a folder on the computer. A session is a terminal running in it.

- `Project.color` is `#rrggbb`. `icon` is an emoji or an SF Symbol name with
  an `sf:` prefix; on Android, map the symbols you support and ignore the
  rest. `logo` is square PNG data; prefer it over `icon`.
- Worktrees have `isWorktree` set, and `parentId` names the project they
  belong to. `isHome` marks the project for the user's home folder.
- `sessions(projectId:)` includes ended sessions whose output the server
  keeps. Attach only to `live` ones.
- `Session.owner` is the kind of client that attached first, if any.
  `attached` tells whether this phone is attached.
- `createSession` starts a shell in the project's folder but doesn't attach
  to it. Call `attach` next.
- `endSession` ends the session for everyone, and its pane closes on the
  desktop too. Ask the user first.

## Terminals

`attach(sessionId:columns:rows:)` returns a `Terminal`, and `screen()` returns
its current contents. Attach sets the session's size only when no other client
is showing it. When the desktop already shows the session, it keeps the
desktop's size. Always lay out `screen.columns` by `screen.rows` cells, and let
the user zoom or scroll sideways when that is wider than the phone.

`screenChanged` can arrive many times a second. Set a flag, then read
`screen()` and draw once per display frame, using `CADisplayLink` on iOS or
`Choreographer` on Android.

Drawing:

- `lines` run top to bottom and `spans` run left to right. Advance by
  `span.width`, which counts terminal cells, not characters. Wide characters
  such as CJK and most emoji take two cells.
- A line can end before the last column. The rest of the row is blank, in the
  default background.
- `default` means the theme's foreground or background. `indexed` 0–15 are the
  theme's ANSI colors, and 16–255 follow the standard xterm palette. `rgb` is
  exact. The app owns the theme.
- `inverse` swaps foreground and background. `invisible` draws only the
  background. `faint` dims the text.
- `cursor` has a row, a column, a `visible` flag, and a shape.
- `title` is the title the program set. `directory` is the shell's current
  folder.

```swift
for (row, line) in screen.lines.enumerated() {
    var column = 0
    for span in line.spans {
        var foreground = color(span.style.foreground, fallback: theme.foreground)
        var background = color(span.style.background, fallback: theme.background)
        if span.style.inverse { swap(&foreground, &background) }
        fill(row: row, column: column, cells: Int(span.width), background)
        if !span.style.invisible {
            draw(span.text, row: row, column: column, foreground, span.style)
        }
        column += Int(span.width)
    }
}
```

```kotlin
screen.lines.forEachIndexed { row, line ->
    var column = 0
    for (span in line.spans) {
        var foreground = color(span.style.foreground, theme.foreground)
        var background = color(span.style.background, theme.background)
        if (span.style.inverse) foreground = background.also { background = foreground }
        fill(row, column, span.width.toInt(), background)
        if (!span.style.invisible) draw(span.text, row, column, foreground, span.style)
        column += span.width.toInt()
    }
}
```

After `sessionEnded`, `screen()` still returns the last contents, but input
fails. `detach()` stops updates to this phone; the session keeps running on the
computer.

## Input

Input goes to the session itself, so every client showing it sees the result.

| The user | Call |
| --- | --- |
| Types text | `sendInput(bytes:)` with UTF-8 |
| Presses Return, Tab, Esc, or Backspace | `sendKey` with `enter`, `tab`, `escape`, or `backspace` |
| Uses the arrow keys, Home, End, Page Up, or Page Down | `sendKey` with `up`, `down`, `left`, `right`, `home`, `end`, `pageUp`, or `pageDown` |
| Uses Insert, Delete, Shift-Tab, or F1–F12 | `sendKey` with `insert`, `delete`, `backTab`, or `function(number)` |
| Holds Ctrl or Alt on the accessory bar | `sendKey(key: .character(text:), modifiers:)`. With Control, only the first character counts |
| Pastes | `paste(text:)` |

```swift
try await SDK.run { try terminal.sendInput(bytes: Data("git status".utf8)) }
try await SDK.run { try terminal.sendKey(key: .enter, modifiers: Modifiers(shift: false, alt: false, control: false)) }
try await SDK.run { try terminal.sendKey(key: .character(text: "c"), modifiers: Modifiers(shift: false, alt: false, control: true)) }
```

```kotlin
val none = Modifiers(shift = false, alt = false, control = false)
withContext(sdk) {
    terminal.sendInput("git status".toByteArray())
    terminal.sendKey(Key.Enter, none)
    terminal.sendKey(Key.Character("c"), none.copy(control = true))   // Ctrl-C
}
```

- `sendKey` encodes keys the way the running program expects, including its
  cursor-key mode. Enter sends a carriage return, and Backspace sends DEL.
- `paste` turns line breaks into Return and removes escape characters, so
  pasted text can't act as typed commands. It marks the text as a paste when
  the program asks for that.
- One call carries at most 1 MiB.
- `resize(columns:rows:)` resizes the session for every client, and the
  desktop's pane reflows to match. Offer it as an explicit action, such as
  "Fit to phone".

## Scrollback

`screen()` shows only the visible rows. When the user scrolls up and
`screen.historyRows` is above zero, take a `Scrollback`: a fixed copy of the
newest history rows followed by the screen. New output keeps updating the live
screen but never moves the copy.

```swift
let scrollback = try await SDK.run { try terminal.scrollback(maxRows: 200) }
var lines = scrollback.lines()                       // oldest first

let older = try await SDK.run { try scrollback.loadOlder(maxRows: 500) }
lines.insert(contentsOf: older, at: 0)               // empty when nothing older remains
```

```kotlin
val scrollback = withContext(sdk) { terminal.scrollback(200u) }
val lines = scrollback.lines().toMutableList()       // oldest first

val older = withContext(sdk) { scrollback.loadOlder(500u) }
lines.addAll(0, older)                               // empty when nothing older remains
```

- `maxRows` is clamped to 1–500, and a page can hold fewer rows than
  requested.
  Keep the user's place by moving the scroll offset down by the number of rows
  added.
- `loadOlder` also returns nothing once the history is cleared or rewrapped on
  the computer. Take a new scrollback to see it again.
- When the user returns to the bottom, drop the scrollback and show the live
  screen.

## Taps and scrolling

Full-screen programs such as editors, pagers, and `htop` can take mouse input.
`screen()` says when one does. Route gestures the way the desktop does:

| The user | When | Call |
| --- | --- | --- |
| Scrolls | `mouseTracking` or `alternateScroll` is on | `scroll(direction:row:column:)` |
| Scrolls | Both are off | Scroll a [Scrollback](#scrollback) |
| Taps | `mouseTracking` is on | `click(button:row:column:modifiers:)` with `left` |
| Taps | `mouseTracking` is off | Handle the tap in the app |

```swift
let none = Modifiers(shift: false, alt: false, control: false)
try await SDK.run { try terminal.click(button: .left, row: 3, column: 10, modifiers: none) }
try await SDK.run { try terminal.scroll(direction: .up, row: 3, column: 10) }
```

```kotlin
val none = Modifiers(shift = false, alt = false, control = false)
withContext(sdk) {
    terminal.click(MouseButton.LEFT, 3u, 10u, none)
    terminal.scroll(ScrollDirection.UP, 3u, 10u)
}
```

- `mouseTracking` means the program reads the mouse. Otherwise,
  `alternateScroll` means it reads scrolling as arrow keys, as pagers such as
  `less` do.
- `row` and `column` are the cell under the finger, counted from 0 at the top
  left, like `cursor`.
- Each `scroll` is one mouse-wheel step, which most programs treat as about
  three lines. The desktop sends one step per row scrolled. `up` moves toward
  earlier output, as dragging down does.
- `click` sends a press and then a release. Use `right` or `middle` for other
  gestures, such as a long press.
- When either mode turns on while the user is in a scrollback, drop it and
  show the live screen.

## Agent activity

Muxy tracks AI coding agents running in sessions. `activity()` returns each
agent's state and recent notifications.

- `Agent.provider` is a display name such as "Claude Code" or "Codex".
  `blocked` means the agent is waiting for the user.
- An `attention` event means an agent needs the user. A `completed` event
  means it finished.
- `acknowledgeActivity(eventIds:)` marks events as seen on every client,
  including the desktop.
- The phone learns about activity only while it's connected. There are no
  push notifications.

## Git and files

`git(projectId:action:)` and `files(projectId:action:)` do everything the
desktop does with a project's repository and folder: status and diffs,
staging, commits, branches, pushes and pulls, worktrees, pull requests, and
reading and editing files. Their cases match the protocol's `GitAction`,
`GitReply`, `FilesAction`, and `FilesReply` in `crates/muxy-protocol/src`:
`git.rs`, `git/extension.rs`, and `files.rs`. Read those for what each action
does.

```swift
if case .changes(let files) = try connection.git(projectId: project.id, action: .changes) {
    show(files)
}
_ = try connection.files(projectId: project.id, action: .write(path: "notes.md", content: text))
```

```kotlin
val reply = connection.git(project.id, GitAction.Changes)
if (reply is GitReply.Changes) show(reply.files)
connection.files(project.id, FilesAction.Write("notes.md", text))
```

- Run these off the main thread on a queue of their own. A push or pull can
  take minutes, and keystrokes shouldn't wait behind it.
- File paths are relative to the project's folder, and `""` is the folder
  itself. Git's file paths are relative to the repository's root, which is the
  same unless the project is a folder inside a repository. Directories, such
  as a worktree's, are absolute paths on the computer.
- `read` returns UTF-8 text of up to 5 MiB; other files fail with
  `Server(reason)`. `delete` moves files to the computer's Trash.
- `GitAction.watch` follows one project's repository per connection; a new
  watch replaces it. A folder without a repository needs a new watch after
  `init`. `FilesAction.watch` follows up to 32 projects until `unwatch`.
  Watches end with the connection.
- Differences from the protocol: ids and paths are strings, so names that
  aren't UTF-8 can't be used. `GitFile.index` and `worktree` are one-letter
  strings, such as `M`, `?`, or a space for unchanged.
  `FilesAction.listDirectory` is the protocol's `List`. The SDK chooses the ids
  of operations and of new worktree projects.

## Reconnecting

- Connect when the app comes to the foreground, and disconnect when it goes to
  the background.
- After `serverRestarting`, show "Reconnecting…" and try again a second or two
  later.
- After an unexpected `disconnected` while the app is in the foreground, retry
  with backoff: 1 s, 2 s, 5 s, then every 10 s.
- Stop retrying on `Unauthorized`, `IdentityMismatch`, `InvalidCredential`, and
  `IncompatibleVersion`. Only the user can fix those.
- After reconnecting, attach again to the sessions the user had open, watch
  again, and read projects, sessions, activity, and what you show from Git and
  files again.

## Errors

The generated error descriptions aren't meant for users, so write your own text
for each case.

| Case | When | Tell the user | Retry |
| --- | --- | --- | --- |
| `InvalidLink` | The text isn't a Muxy pairing link | "This isn't a Muxy pairing code." | No |
| `InvalidCredential` | The saved credential is damaged | "Pair this phone again." | No |
| `Unreachable(reason)` | No address answered. The computer may be asleep or on another network, mobile access may be off, or a firewall or a declined local network permission blocked it | "Can't reach *serverName*. Check that it's awake and on the same network or VPN." | Yes |
| `IdentityMismatch` | A server with a different certificate answered | "This computer's identity changed. Pair again." | No |
| `Unauthorized` | The phone was revoked, or while pairing, the code expired, was used, or was replaced | "This phone isn't paired anymore." or "Show a new code on your computer." | No |
| `IncompatibleVersion` | The app's SDK and the server have different compatibility identifiers | "Update Muxy on your phone or computer." | No |
| `Timeout` | The server didn't answer in time | "Muxy isn't responding." | Yes |
| `Disconnected` | The connection closed during the call | Reconnect | Yes |
| `Server(reason)` | The server refused the request; `reason` is readable text | Show `reason` | Depends |

```swift
do {
    connection = try await SDK.run { try Connection.connect(credential: credential, listener: events) }
} catch MobileError.Unreachable(let reason) {
    showOffline(detail: reason)
} catch MobileError.Unauthorized, MobileError.IdentityMismatch, MobileError.InvalidCredential {
    askToPairAgain()
} catch MobileError.IncompatibleVersion {
    askToUpdate()
} catch {
    showRetry(error)
}
```

```kotlin
try {
    connection = withContext(sdk) { Connection.connect(credential, events) }
} catch (error: MobileException) {
    when (error) {
        is MobileException.Unreachable -> showOffline(error.reason)
        is MobileException.Unauthorized,
        is MobileException.IdentityMismatch,
        is MobileException.InvalidCredential -> askToPairAgain()
        is MobileException.IncompatibleVersion -> askToUpdate()
        else -> showRetry(error)
    }
}
```

## Limits

| What | Value |
| --- | --- |
| Default port | 7419, which the user can change |
| Network | IPv4, on the local network or through a VPN |
| Pairing code | Single use, 5 minutes, replaced by a newer code |
| Addresses in a pairing code | Up to 8 |
| Paired devices per computer | 64 |
| Device name | Up to 64 bytes |
| Connect timeout | 4 s per address |
| Input per call | 1 MiB |
| Scrollback page | 1–500 rows |

## Security

- A paired phone can do anything a terminal on that computer can. Treat the
  token like a password, and consider an app lock such as Face ID before
  connecting.
- The SDK talks only to a server whose certificate matches the pairing code,
  so another machine can't pose as the computer.
- Phones can't turn mobile access on or off, pair or revoke devices, stop the
  server, change server settings, or run extension commands. Those stay on the
  computer.
- Revoking a phone in Settings → Mobile closes its connection and invalidates
  its token at once.
- The desktop withdraws its pairing code when Settings closes, and the CLI
  withdraws its code when `muxy mobile pair` exits.

## Testing against a server

Run a server on a Mac from the same Muxy commit as the SDK. In the desktop
app, use Settings → Mobile; **Copy Link** copies the pairing link for pasting
into a simulator. Without the desktop app, use the CLI:

```sh
cargo build -p muxy-server -p muxy-cli
target/debug/muxy mobile enable          # listen on 7419; add --port 7500 to change it
target/debug/muxy mobile pair            # prints the QR code and link, then waits for the phone
target/debug/muxy mobile                 # status and paired devices
target/debug/muxy mobile revoke 3f2a     # revoke a device by the start of its id
target/debug/muxy mobile disable
```

- The first time, macOS asks whether `muxy-server` may accept incoming
  connections. Allow it.
- The iOS Simulator and the Android Emulator reach the Mac through the LAN
  address in the pairing link.
- `cargo test -p muxy-mobile` pairs, connects, attaches, types, scrolls back,
  sends taps and scrolling, works with Git and files, and revokes against a
  real server.

## Not in this version

- Search
- Mouse drags and inline images
- Push notifications
- Finding computers automatically; pairing always starts from the QR code
- IPv6, and access from outside the network without a VPN
