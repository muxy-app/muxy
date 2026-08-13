import Darwin
import Testing

@testable import Muxy

@Suite("ProcessArgumentsInspector")
struct ProcessArgumentsInspectorTests {
    @Test("resolves the current process invocation")
    func resolvesCurrentProcessInvocation() throws {
        let invocation = try #require(ProcessArgumentsInspector.invocation(pid: UInt64(getpid())))
        #expect(!invocation.executablePath.isEmpty)
        #expect(!invocation.arguments.isEmpty)
        #expect(invocation.workingDirectory != nil)
    }

    @Test("rejects invalid process identifiers")
    func rejectsInvalidProcessIdentifiers() {
        #expect(ProcessArgumentsInspector.invocation(pid: 0) == nil)
        #expect(ProcessArgumentsInspector.invocation(pid: UInt64(Int32.max) + 1) == nil)
    }
}
