import Foundation
import Testing

@testable import Muxy

@Suite("LoginShellPath")
struct LoginShellPathTests {
    @Test("hydrate waits for resolved login shell PATH")
    func hydrateWaitsForResolvedLoginShellPath() async {
        let path = LoginShellPath()

        await path.hydrate {
            "/tmp/custom-bin:/usr/bin"
        }

        #expect(path.value == "/tmp/custom-bin:/usr/bin")
    }

    @Test("hydrate keeps default PATH when lookup fails")
    func hydrateKeepsDefaultPathWhenLookupFails() async {
        let path = LoginShellPath()

        await path.hydrate {
            nil
        }

        #expect(path.value == LoginShellPath.defaultPath)
    }

    @Test("environment hydration captures PATH and Copilot home together")
    func hydrateCapturesLoginShellEnvironment() async {
        let path = LoginShellPath()

        await path.hydrateEnvironment {
            LoginShellEnvironmentValues(
                path: "/tmp/custom-bin:/usr/bin",
                copilotHome: "/tmp/custom-copilot"
            )
        }

        #expect(path.value == "/tmp/custom-bin:/usr/bin")
        #expect(path.copilotHome == "/tmp/custom-copilot")
    }

    @Test("environment hydration coalesces concurrent callers")
    func environmentHydrationCoalescesConcurrentCallers() async {
        let path = LoginShellPath()
        let reader = GatedLoginShellEnvironmentReader()
        let unusedReader = LoginShellEnvironmentReader()

        let first = path.environmentHydrationTask(
            forceRefresh: true,
            readFromLoginShell: reader.read
        )
        let started = await reader.waitUntilRead()
        guard started else {
            reader.releaseAll()
            #expect(Bool(false), "The hydration reader did not start")
            return
        }
        let second = path.environmentHydrationTask(
            forceRefresh: true,
            readFromLoginShell: unusedReader.read
        )
        reader.releaseAll()
        await first?.value
        await second?.value

        #expect(reader.count == 1)
        #expect(unusedReader.count == 0)
        #expect(path.value == "/tmp/custom-bin:/usr/bin")
    }

    @Test("environment hydration refreshes a resolved environment")
    func environmentHydrationRefreshesResolvedEnvironment() async {
        let path = LoginShellPath()
        let firstReader = LoginShellEnvironmentReader(path: "/tmp/first-bin:/usr/bin")
        let secondReader = LoginShellEnvironmentReader(path: "/tmp/second-bin:/usr/bin")

        await path.hydrateEnvironment(readFromLoginShell: firstReader.read)
        await path.hydrateEnvironment(readFromLoginShell: secondReader.read)

        #expect(firstReader.count == 1)
        #expect(secondReader.count == 1)
        #expect(path.value == "/tmp/second-bin:/usr/bin")
    }

    @Test("cached environment hydration awaits an active refresh")
    func cachedEnvironmentHydrationAwaitsActiveRefresh() async {
        let path = LoginShellPath()
        let initialReader = LoginShellEnvironmentReader(path: "/tmp/initial-bin:/usr/bin")
        let refreshReader = GatedLoginShellEnvironmentReader(path: "/tmp/refreshed-bin:/usr/bin")
        let unusedReader = LoginShellEnvironmentReader()

        await path.hydrateEnvironment(readFromLoginShell: initialReader.read)
        let refresh = path.environmentHydrationTask(
            forceRefresh: true,
            readFromLoginShell: refreshReader.read
        )
        let started = await refreshReader.waitUntilRead()
        guard started else {
            refreshReader.releaseAll()
            #expect(Bool(false), "The refresh reader did not start")
            return
        }
        let cachedRequest = path.environmentHydrationTask(
            forceRefresh: false,
            readFromLoginShell: unusedReader.read
        )
        refreshReader.releaseAll()
        await refresh?.value
        await cachedRequest?.value

        #expect(unusedReader.count == 0)
        #expect(path.value == "/tmp/refreshed-bin:/usr/bin")
    }

    @Test("environment hydration retries after a failed lookup")
    func environmentHydrationRetriesAfterFailure() async {
        let path = LoginShellPath()
        let reader = LoginShellEnvironmentReader()

        await path.hydrateEnvironment(forceRefresh: false) { nil }
        await path.hydrateEnvironment(forceRefresh: false, readFromLoginShell: reader.read)
        await path.hydrateEnvironment(forceRefresh: false, readFromLoginShell: reader.read)

        #expect(reader.count == 1)
        #expect(path.value == "/tmp/custom-bin:/usr/bin")
    }

    @Test("environment hydration stops waiting when its timeout expires")
    func environmentHydrationStopsWaitingAtTimeout() async {
        let path = LoginShellPath()
        let reader = GatedLoginShellEnvironmentReader()
        let clock = ContinuousClock()
        let hydration = Task {
            try await path.hydrateEnvironmentIfNeeded(
                timeout: 0.05,
                readFromLoginShell: reader.read
            )
        }
        let started = await reader.waitUntilRead()
        guard started else {
            reader.releaseAll()
            #expect(Bool(false), "The hydration reader did not start")
            return
        }
        let startedAt = clock.now

        await #expect(throws: AsyncTimeoutError.self) {
            try await hydration.value
        }
        let elapsed = startedAt.duration(to: clock.now)
        reader.releaseAll()

        #expect(elapsed < .seconds(1))
    }

    @Test("login shell lookup loads interactive configuration")
    func loginShellLookupLoadsInteractiveConfiguration() {
        #expect(LoginShellPath.shellArguments.prefix(3) == ["-l", "-i", "-c"])
        #expect(LoginShellPath.shellArguments.last?.contains("printenv COPILOT_HOME") == true)
    }

    @Test("login shell lookup extracts PATH without startup output")
    func loginShellLookupExtractsPathWithoutStartupOutput() {
        let output = """
        shell startup output
        __MUXY_PATH_START__/custom/bin:/usr/bin
        __MUXY_PATH_END__
        """

        #expect(LoginShellPath.extractPath(from: output) == "/custom/bin:/usr/bin")
    }

    @Test("login shell lookup extracts the complete environment")
    func loginShellLookupExtractsEnvironment() {
        let output = """
        shell startup output
        __MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__
        __MUXY_COPILOT_HOME_START__/custom/copilot__MUXY_COPILOT_HOME_END__
        """

        #expect(LoginShellPath.extractEnvironment(from: output) == LoginShellEnvironmentValues(
            path: "/custom/bin:/usr/bin",
            copilotHome: "/custom/copilot"
        ))
    }

    @Test("login shell lookup accepts an unset Copilot home")
    func loginShellLookupAcceptsUnsetCopilotHome() {
        let output = """
        __MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__
        __MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__
        """

        #expect(LoginShellPath.extractEnvironment(from: output) == LoginShellEnvironmentValues(
            path: "/custom/bin:/usr/bin",
            copilotHome: nil
        ))
    }

    @Test("login shell lookup rejects malformed output")
    func loginShellLookupRejectsMalformedOutput() {
        #expect(LoginShellPath.extractPath(from: "/custom/bin:/usr/bin") == nil)
        #expect(LoginShellPath.extractPath(from: "__MUXY_PATH_START____MUXY_PATH_END__") == nil)
        #expect(LoginShellPath.extractEnvironment(
            from: "__MUXY_PATH_START__/usr/bin__MUXY_PATH_END__"
        ) == nil)
    }

    @Test("login shell lookup drains noisy startup output")
    func loginShellLookupDrainsNoisyStartupOutput() {
        let command = """
        /usr/bin/yes stdout | /usr/bin/head -c 400000
        /usr/bin/yes stderr | /usr/bin/head -c 400000 >&2
        printf '__MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__'
        """

        let path = LoginShellPath.readPath(
            shellPath: "/bin/sh",
            arguments: ["-c", command],
            timeout: .seconds(30)
        )

        #expect(path == "/custom/bin:/usr/bin")
    }

    @Test("login shell lookup does not wait for descendants holding output pipes")
    func loginShellLookupDoesNotWaitForDescendantOutput() {
        let command = """
        /bin/sleep 1 &
        printf '__MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__'
        """

        let path = LoginShellPath.readPath(
            shellPath: "/bin/sh",
            arguments: ["-c", command],
            timeout: .milliseconds(100)
        )

        #expect(path == nil)
    }

    @Test("login shell lookup force terminates a stalled process")
    func loginShellLookupForceTerminatesStalledProcess() {
        let clock = ContinuousClock()
        let started = clock.now

        let path = LoginShellPath.readPath(
            shellPath: "/bin/sh",
            arguments: ["-c", "trap '' TERM; exec /bin/sleep 10"],
            timeout: .milliseconds(100)
        )

        #expect(path == nil)
        #expect(started.duration(to: clock.now) < .seconds(5))
    }

    @Test("login shell lookup tolerates a truncated UTF-8 scalar before PATH")
    func loginShellLookupToleratesTruncatedUTF8() {
        let output = Data([0x80]) + Data(
            "__MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__".utf8
        )

        #expect(LoginShellPath.extractPath(from: output) == "/custom/bin:/usr/bin")
    }
}

private final class LoginShellEnvironmentReader: @unchecked Sendable {
    private let lock = NSLock()
    private let path: String
    private var calls = 0

    init(path: String = "/tmp/custom-bin:/usr/bin") {
        self.path = path
    }

    var count: Int {
        lock.withLock { calls }
    }

    func read() -> LoginShellEnvironmentValues? {
        lock.withLock { calls += 1 }
        return LoginShellEnvironmentValues(path: path, copilotHome: nil)
    }
}

private final class GatedLoginShellEnvironmentReader: @unchecked Sendable {
    private let lock = NSLock()
    private let started = DispatchSemaphore(value: 0)
    private let releaseCondition = NSCondition()
    private let path: String
    private var calls = 0
    private var released = false

    init(path: String = "/tmp/custom-bin:/usr/bin") {
        self.path = path
    }

    var count: Int {
        lock.withLock { calls }
    }

    func read() -> LoginShellEnvironmentValues? {
        lock.withLock { calls += 1 }
        started.signal()
        releaseCondition.lock()
        while !released {
            releaseCondition.wait()
        }
        releaseCondition.unlock()
        return LoginShellEnvironmentValues(path: path, copilotHome: nil)
    }

    func waitUntilRead() async -> Bool {
        await Task.detached { [self] in
            waitForStart()
        }.value
    }

    func releaseAll() {
        releaseCondition.lock()
        released = true
        releaseCondition.broadcast()
        releaseCondition.unlock()
    }

    private func waitForStart() -> Bool {
        started.wait(timeout: .now() + 5) == .success
    }
}
