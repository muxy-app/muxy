# Low Memory Mode Review Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken `%output` parser, eliminate per-keystroke process spawning, and document the tmux architecture contract.

**Architecture:** `TmuxCaptureService` has two codepaths: a persistent control mode process (for streaming panes) and CLI fallback (for non-streaming). The fix routes `sendInput`/`resizeSession`/`scroll` through the existing control mode process's stdin when available, falling back to CLI otherwise. The `%output` parser is rewritten from base64 to tmux's actual octal `\xxx` escape format.

**Tech Stack:** Swift 6.0+, Swift Testing framework, Foundation `Process`/`Pipe`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `Muxy/Services/TmuxCaptureService.swift` | Fix parser, add stdin command routing, add doc comments |
| Modify | `Tests/MuxyTests/Services/TmuxCaptureServiceTests.swift` | Rewrite tests for real tmux format |
| Modify | `Muxy/Services/TmuxConfiguration.swift` | Add doc comments |

---

### Task 1: Write failing tests for octal escape parser

**Files:**
- Modify: `Tests/MuxyTests/Services/TmuxCaptureServiceTests.swift:29-108`

- [ ] **Step 1: Replace the entire `TmuxControlOutputParsingTests` suite with octal-escape-based tests**

Replace lines 29–108 in `Tests/MuxyTests/Services/TmuxCaptureServiceTests.swift` with:

```swift
@Suite("TmuxCaptureService.parseControlOutput")
struct TmuxControlOutputParsingTests {
    @Test("parses single %output line with plain text")
    func singlePlainOutput() {
        let input = "%output %1 Hello World\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(results[0] == Data("Hello World".utf8))
    }

    @Test("parses %output with octal-escaped carriage return and newline")
    func octalEscapedOutput() {
        let input = "%output %0 Hello\015\012World\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        let expected = Data("Hello\r\nWorld".utf8)
        #expect(results[0] == expected)
    }

    @Test("parses %output with ANSI escape sequence")
    func ansiEscapeOutput() {
        let input = "%output %0 \033[31mRed\033[0m\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        let expected = Data("\u{1B}[31mRed\u{1B}[0m".utf8)
        #expect(results[0] == expected)
    }

    @Test("parses multiple %output lines")
    func multipleOutputLines() {
        let input = "%output %1 abc\015\012\n%output %2 xyz\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 2)
        #expect(results[0] == Data("abc\r\n".utf8))
        #expect(results[1] == Data("xyz".utf8))
    }

    @Test("skips non-%output lines")
    func skipsNonOutputLines() {
        let input = "%layout %1 80x24\n%output %2 data\n%exit\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(results[0] == Data("data".utf8))
    }

    @Test("returns empty for %output with missing payload")
    func missingPayload() {
        let input = "%output %1\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("returns empty for non-UTF8 input")
    func nonUTF8Input() {
        let data = Data([0xFF, 0xFE, 0xFD])
        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("returns empty for empty input")
    func emptyInput() {
        let data = Data()
        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("handles payload with spaces")
    func payloadWithSpaces() {
        let input = "%output %1 hello world\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(String(data: results[0], encoding: .utf8) == "hello world")
    }

    @Test("handles octal escape for null byte")
    func octalNullByte() {
        let input = "%output %0 a\000b\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        var expected = Data("a".utf8)
        expected.append(0x00)
        expected.append(Data("b".utf8))
        #expect(results[0] == expected)
    }

    @Test("handles multiple consecutive octal escapes")
    func consecutiveOctalEscapes() {
        let input = "%output %0 \015\012\033[31m\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        let expected = Data("\r\n\u{1B}[31m".utf8)
        #expect(results[0] == expected)
    }

    @Test("handles backslash followed by non-octal as literal backslash")
    func backslashNonOctal() {
        let input = "%output %0 path\\to\\file\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(String(data: results[0], encoding: .utf8) == "path\\to\\file")
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `swift test --filter TmuxControlOutputParsingTests 2>&1 | tail -20`
Expected: Multiple test failures — the current parser tries base64 decode on plain text like `"Hello World"` and returns empty.

- [ ] **Step 3: Commit**

```bash
git add Tests/MuxyTests/Services/TmuxCaptureServiceTests.swift
git commit -m "test: rewrite parseControlOutput tests for real tmux octal format"
```

---

### Task 2: Fix `parseControlOutput` to decode octal escapes

**Files:**
- Modify: `Muxy/Services/TmuxCaptureService.swift:225-243`

- [ ] **Step 1: Replace the `parseControlOutput` method**

Replace lines 225–243 in `Muxy/Services/TmuxCaptureService.swift` with:

```swift
nonisolated static func parseControlOutput(_ data: Data) -> [Data] {
    guard let output = String(data: data, encoding: .utf8) else { return [] }
    var results: [Data] = []

    for line in output.split(separator: "\n") {
        let lineStr = String(line)
        guard lineStr.hasPrefix("%output") else { continue }

        let withoutPrefix = lineStr.dropFirst("%output ".count)
        let parts = withoutPrefix.split(separator: " ", maxSplits: 1)
        guard parts.count == 2 else { continue }

        let payload = String(parts[1])
        results.append(decodeOctalEscapes(payload))
    }

    return results
}

nonisolated private static func decodeOctalEscapes(_ input: String) -> Data {
    var result = Data()
    var chars = input[...]

    while !chars.isEmpty {
        if chars.first == "\\" {
            let afterBackslash = chars.dropFirst()
            guard afterBackslash.count >= 3 else {
                result.append(Data("\\\(".init(afterBackslash))".utf8))
                break
            }
            let octalChars = afterBackslash.prefix(3)
            if let byte = UInt8(octalChars, radix: 8) {
                result.append(byte)
                chars = afterBackslash.dropFirst(3)
            } else {
                result.append(0x5C)
                chars = afterBackslash
            }
        } else {
            result.append(chars.first!.asciiValue!)
            chars = chars.dropFirst()
        }
    }

    return result
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `swift test --filter TmuxControlOutputParsingTests 2>&1 | tail -20`
Expected: All 12 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add Muxy/Services/TmuxCaptureService.swift
git commit -m "fix: parse tmux %output as octal escapes instead of base64"
```

---

### Task 3: Add stdin command sending to `TmuxControlModeProcess`

**Files:**
- Modify: `Muxy/Services/TmuxCaptureService.swift:246-338`

- [ ] **Step 1: Add stdin pipe and `send` method to `TmuxControlModeProcess`**

In the `TmuxControlModeProcess` class (line 246), make these changes:

1. Add a `_inputPipe` stored property after `_outputPipe` (around line 253):
```swift
private var _inputPipe: Pipe?
```

2. In `start()` (line 267), before `process.standardOutput = pipe`, add the input pipe:
```swift
let inputPipe = Pipe()
process.standardInput = inputPipe
_inputPipe = inputPipe
```

3. Add a `send` method after `clearRunningFlag` (after line 337):
```swift
func send(_ command: String) {
    lock.lock()
    let pipe = _inputPipe
    lock.unlock()

    guard let pipe else { return }
    guard let data = (command + "\n").data(using: .utf8) else { return }
    pipe.fileHandleForWriting.write(data)
}
```

4. In `stop()` (line 314), add cleanup for `_inputPipe` alongside the existing cleanup. After `let pipe = _outputPipe` add:
```swift
let inputPipe = _inputPipe
_inputPipe = nil
```

And after `pipe?.fileHandleForReading.readabilityHandler = nil` add:
```swift
inputPipe?.fileHandleForWriting.closeFile()
```

- [ ] **Step 2: Build to verify compilation**

Run: `swift build 2>&1 | tail -10`
Expected: BUILD SUCCEEDED

- [ ] **Step 3: Commit**

```bash
git add Muxy/Services/TmuxCaptureService.swift
git commit -m "feat: add stdin command sending to TmuxControlModeProcess"
```

---

### Task 4: Route `sendInput`/`resizeSession`/`scroll` through control mode stdin

**Files:**
- Modify: `Muxy/Services/TmuxCaptureService.swift:138-223`

- [ ] **Step 1: Replace `sendInput` with control-mode-first routing**

Replace lines 138–166 with:

```swift
func sendInput(paneID: UUID, bytes: Data) {
    if let process = streamingProcesses[paneID] {
        guard let text = String(data: bytes, encoding: .utf8) ?? String(data: bytes, encoding: .ascii) else {
            return
        }
        process.send("send-keys -t \(TmuxConfiguration.sessionName(for: paneID)) -l \(text)")
        return
    }

    guard let tmux = TmuxConfiguration.findBinary() else { return }
    let session = TmuxConfiguration.sessionName(for: paneID)
    let socket = TmuxConfiguration.socketName

    guard let text = String(data: bytes, encoding: .utf8) ?? String(data: bytes, encoding: .ascii) else {
        return
    }

    let process = Process()
    process.executableURL = URL(fileURLWithPath: tmux)
    process.arguments = [
        "-L", socket,
        "send-keys",
        "-t", session,
        "-l", text,
    ]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice

    DispatchQueue.global(qos: .userInteractive).async {
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            logger.error("tmux send-keys failed: \(error.localizedDescription)")
        }
    }
}
```

- [ ] **Step 2: Replace `resizeSession` with control-mode-first routing**

Replace lines 168–193 with:

```swift
func resizeSession(paneID: UUID, cols: UInt32, rows: UInt32) {
    if let process = streamingProcesses[paneID] {
        process.send("refresh-client -C \(cols)x\(rows)")
        return
    }

    guard let tmux = TmuxConfiguration.findBinary() else { return }
    let session = TmuxConfiguration.sessionName(for: paneID)
    let socket = TmuxConfiguration.socketName

    let process = Process()
    process.executableURL = URL(fileURLWithPath: tmux)
    process.arguments = [
        "-L", socket,
        "resize-window",
        "-t", session,
        "-x", "\(cols)",
        "-y", "\(rows)",
    ]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice

    DispatchQueue.global(qos: .utility).async {
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            logger.error("tmux resize-window failed: \(error.localizedDescription)")
        }
    }
}
```

- [ ] **Step 3: Replace `scroll` with control-mode-first routing**

Replace lines 195–223 with:

```swift
func scroll(paneID: UUID, deltaY: Double) {
    let key = deltaY > 0 ? "Up" : "Down"
    let lines = min(max(1, Int(abs(deltaY))), 20)

    if let process = streamingProcesses[paneID] {
        process.send("send-keys -t \(TmuxConfiguration.sessionName(for: paneID)) -N \(lines) \(key)")
        return
    }

    guard let tmux = TmuxConfiguration.findBinary() else { return }
    let session = TmuxConfiguration.sessionName(for: paneID)
    let socket = TmuxConfiguration.socketName

    let process = Process()
    process.executableURL = URL(fileURLWithPath: tmux)
    process.arguments = [
        "-L", socket,
        "send-keys",
        "-t", session,
        "-N", "\(lines)",
        key,
    ]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice

    DispatchQueue.global(qos: .utility).async {
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            logger.error("tmux scroll failed: \(error.localizedDescription)")
        }
    }
}
```

- [ ] **Step 4: Build to verify compilation**

Run: `swift build 2>&1 | tail -10`
Expected: BUILD SUCCEEDED

- [ ] **Step 5: Run all TmuxCaptureService tests**

Run: `swift test --filter TmuxCaptureService 2>&1 | tail -20`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add Muxy/Services/TmuxCaptureService.swift
git commit -m "feat: route sendInput/resize/scroll through control mode stdin"
```

---

### Task 5: Add architecture doc comments

**Files:**
- Modify: `Muxy/Services/TmuxConfiguration.swift`
- Modify: `Muxy/Services/TmuxCaptureService.swift`

- [ ] **Step 1: Add doc comment to `TmuxConfiguration` enum**

Add above `enum TmuxConfiguration {` (line 6) in `Muxy/Services/TmuxConfiguration.swift`:

```swift
/// Manages tmux binary discovery and session naming for Low Memory Mode.
///
/// Low Memory Mode reduces RAM by offloading hidden terminal surfaces to tmux sessions.
/// Shell state persists across workspace switches without keeping Ghostty surfaces in memory.
///
/// Requires tmux 3.3+ installed via Homebrew (`brew install tmux`).
/// When tmux is absent, the feature degrades gracefully — the settings toggle is disabled
/// and all terminals use the standard Ghostty rendering path.
```

- [ ] **Step 2: Add doc comment to `TmuxCaptureService` class**

Add above `final class TmuxCaptureService {` (line 7) in `Muxy/Services/TmuxCaptureService.swift`:

```swift
/// Manages tmux sessions for terminal capture and streaming in Low Memory Mode.
///
/// Two codepaths:
/// - **Control mode** (`TmuxControlModeProcess`): persistent tmux `-C` connection per streaming pane.
///   Reads `%output` notifications (octal `\xxx` escaping) and sends commands via stdin.
/// - **CLI fallback**: spawns short-lived `Process()` for non-streaming operations (`captureSnapshot`).
///
/// Streaming panes route `sendInput`, `resizeSession`, and `scroll` through the control mode
/// stdin to avoid per-keystroke process spawning. Non-streaming panes fall back to CLI.
```

- [ ] **Step 3: Build and run full test suite**

Run: `swift build 2>&1 | tail -5 && swift test --filter TmuxCaptureService 2>&1 | tail -20`
Expected: BUILD SUCCEEDED, all tests PASS

- [ ] **Step 4: Commit**

```bash
git add Muxy/Services/TmuxConfiguration.swift Muxy/Services/TmuxCaptureService.swift
git commit -m "docs: add architecture comments for Low Memory Mode tmux dependency"
```

---

### Task 6: Run full checks

**Files:** None

- [ ] **Step 1: Run `scripts/checks.sh --fix`**

Run: `scripts/checks.sh --fix 2>&1 | tail -30`
Expected: All checks pass (format, lint, build, test)

- [ ] **Step 2: Fix any issues and re-run if needed**

If any issues found, fix and re-run `scripts/checks.sh --fix`.
