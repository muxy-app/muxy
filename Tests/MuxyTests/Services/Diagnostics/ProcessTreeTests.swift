import Foundation
import Testing

@testable import Muxy

@Suite("ProcessTree")
struct ProcessTreeTests {
    private let hostName = "MuxyExtensionHost"

    private func proc(_ pid: pid_t, ppid: pid_t, pgid: pid_t = 0, comm: String = "") -> ProcTopology {
        ProcTopology(pid: pid, ppid: ppid, pgid: pgid, comm: comm)
    }

    @Test("descendants walks a linear chain")
    func descendantsChain() {
        let procs = [proc(1, ppid: 0), proc(10, ppid: 1), proc(11, ppid: 10), proc(12, ppid: 11)]
        #expect(ProcessTree.descendants(of: 10, in: procs) == [10, 11, 12])
    }

    @Test("descendants covers a branching tree")
    func descendantsBranching() {
        let procs = [proc(10, ppid: 1), proc(11, ppid: 10), proc(12, ppid: 10), proc(13, ppid: 11)]
        #expect(ProcessTree.descendants(of: 10, in: procs) == [10, 11, 12, 13])
    }

    @Test("descendants is cycle-safe and ignores self-parenting")
    func descendantsCycleSafe() {
        let procs = [proc(10, ppid: 10), proc(11, ppid: 10), proc(10, ppid: 11)]
        #expect(ProcessTree.descendants(of: 10, in: procs).contains(10))
    }

    @Test("descendants of a leaf returns only itself")
    func descendantsLeaf() {
        #expect(ProcessTree.descendants(of: 10, in: [proc(10, ppid: 1)]) == [10])
    }

    @Test("classify marks the root subtree as app when no hosts")
    func classifyAppOnly() {
        let procs = [proc(100, ppid: 1), proc(101, ppid: 100), proc(102, ppid: 101)]
        let groups = ProcessTree.classify(
            rootPID: 100, procs: procs, knownHostPIDs: [], knownHostPGIDs: [], hostBinaryName: hostName
        )
        #expect(groups == [100: .app, 101: .app, 102: .app])
    }

    @Test("classify marks a host and its descendants as extensionHost")
    func classifyExtensionSubtree() {
        let procs = [proc(100, ppid: 1), proc(200, ppid: 100), proc(201, ppid: 200), proc(300, ppid: 100)]
        let groups = ProcessTree.classify(
            rootPID: 100, procs: procs, knownHostPIDs: [200], knownHostPGIDs: [], hostBinaryName: hostName
        )
        #expect(groups[200] == .extensionHost)
        #expect(groups[201] == .extensionHost)
        #expect(groups[100] == .app)
        #expect(groups[300] == .app)
    }

    @Test("classify marks a reparented host with a known pgid as orphan")
    func classifyOrphanByPGID() {
        let procs = [proc(100, ppid: 1), proc(500, ppid: 1, pgid: 200, comm: "MuxyExtensionHos")]
        let groups = ProcessTree.classify(
            rootPID: 100, procs: procs, knownHostPIDs: [], knownHostPGIDs: [200], hostBinaryName: hostName
        )
        #expect(groups[500] == .orphan)
    }

    @Test("classify ignores a known-pgid process whose name is not the host binary")
    func classifyNotOrphanWhenNameMismatch() {
        let procs = [proc(100, ppid: 1), proc(500, ppid: 1, pgid: 200, comm: "someoneElse")]
        let groups = ProcessTree.classify(
            rootPID: 100, procs: procs, knownHostPIDs: [], knownHostPGIDs: [200], hostBinaryName: hostName
        )
        #expect(groups[500] == nil)
    }

    @Test("classify ignores a reparented child that changed its process group")
    func classifyNotOrphanWhenGroupChanged() {
        let procs = [proc(100, ppid: 1), proc(500, ppid: 1, pgid: 999, comm: "MuxyExtensionHos")]
        let groups = ProcessTree.classify(
            rootPID: 100, procs: procs, knownHostPIDs: [], knownHostPGIDs: [200], hostBinaryName: hostName
        )
        #expect(groups[500] == nil)
    }

    @Test("classify keeps subtree classification over orphan for live pids")
    func classifySubtreeWins() {
        let procs = [proc(100, ppid: 1, pgid: 200), proc(101, ppid: 100, pgid: 200, comm: "MuxyExtensionHos")]
        let groups = ProcessTree.classify(
            rootPID: 100, procs: procs, knownHostPIDs: [], knownHostPGIDs: [200], hostBinaryName: hostName
        )
        #expect(groups[101] == .app)
    }
}
