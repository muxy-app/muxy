import Foundation

actor GitRepositoryCheckCoordinator {
    typealias Check = @Sendable (String, WorkspaceContext) async -> Bool

    private struct Key: Hashable {
        let path: String
        let context: WorkspaceContext
    }

    private struct InFlightCheck {
        let task: Task<Bool, Never>
        var waiterIDs: Set<UUID>
    }

    private struct PermitWaiter {
        let id: UUID
        let continuation: CheckedContinuation<Bool, Never>
    }

    private struct PermitPool {
        var activeCheckCount = 0
        var waiters: [PermitWaiter] = []

        var isIdle: Bool { activeCheckCount == 0 && waiters.isEmpty }
    }

    private let maxConcurrentChecksPerContext: Int
    private let check: Check
    private var permitPools: [WorkspaceContext: PermitPool] = [:]
    private var inFlightChecks: [Key: InFlightCheck] = [:]

    init(maxConcurrentChecksPerContext: Int, check: @escaping Check) {
        precondition(maxConcurrentChecksPerContext > 0)
        self.maxConcurrentChecksPerContext = maxConcurrentChecksPerContext
        self.check = check
    }

    func isGitRepository(_ path: String, context: WorkspaceContext) async -> Bool {
        let key = Key(path: path, context: context)
        let waiterID = UUID()
        let task = joinCheck(for: key, waiterID: waiterID)
        return await withTaskCancellationHandler {
            let result = await task.value
            leaveCheck(for: key, waiterID: waiterID)
            return result
        } onCancel: {
            Task { await self.leaveCheck(for: key, waiterID: waiterID) }
        }
    }

    private func joinCheck(for key: Key, waiterID: UUID) -> Task<Bool, Never> {
        if var inFlightCheck = inFlightChecks[key] {
            inFlightCheck.waiterIDs.insert(waiterID)
            inFlightChecks[key] = inFlightCheck
            return inFlightCheck.task
        }
        let task = Task { await runCheck(for: key) }
        inFlightChecks[key] = InFlightCheck(task: task, waiterIDs: [waiterID])
        return task
    }

    private func leaveCheck(for key: Key, waiterID: UUID) {
        guard var inFlightCheck = inFlightChecks[key],
              inFlightCheck.waiterIDs.remove(waiterID) != nil
        else { return }
        guard inFlightCheck.waiterIDs.isEmpty else {
            inFlightChecks[key] = inFlightCheck
            return
        }
        inFlightChecks.removeValue(forKey: key)
        inFlightCheck.task.cancel()
    }

    private func runCheck(for key: Key) async -> Bool {
        guard await acquirePermit(in: key.context) else { return false }
        defer { releasePermit(in: key.context) }
        return await check(key.path, key.context)
    }

    private func acquirePermit(in context: WorkspaceContext) async -> Bool {
        if claimAvailablePermit(in: context) {
            return true
        }
        return await waitForPermit(in: context)
    }

    private func claimAvailablePermit(in context: WorkspaceContext) -> Bool {
        var pool = permitPools[context] ?? PermitPool()
        guard pool.activeCheckCount < maxConcurrentChecksPerContext else { return false }
        pool.activeCheckCount += 1
        storePool(pool, for: context)
        return true
    }

    private func waitForPermit(in context: WorkspaceContext) async -> Bool {
        let waiterID = UUID()
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                enqueuePermitWaiter(PermitWaiter(id: waiterID, continuation: continuation), in: context)
            }
        } onCancel: {
            Task { await self.abandonPermitWait(waiterID: waiterID, in: context) }
        }
    }

    private func enqueuePermitWaiter(_ waiter: PermitWaiter, in context: WorkspaceContext) {
        guard !Task.isCancelled else {
            waiter.continuation.resume(returning: false)
            return
        }
        var pool = permitPools[context] ?? PermitPool()
        pool.waiters.append(waiter)
        storePool(pool, for: context)
    }

    private func abandonPermitWait(waiterID: UUID, in context: WorkspaceContext) {
        guard var pool = permitPools[context],
              let index = pool.waiters.firstIndex(where: { $0.id == waiterID })
        else { return }
        let waiter = pool.waiters.remove(at: index)
        storePool(pool, for: context)
        waiter.continuation.resume(returning: false)
    }

    private func releasePermit(in context: WorkspaceContext) {
        guard var pool = permitPools[context] else { return }
        if let waiter = pool.waiters.first {
            pool.waiters.removeFirst()
            storePool(pool, for: context)
            waiter.continuation.resume(returning: true)
            return
        }
        pool.activeCheckCount -= 1
        storePool(pool, for: context)
    }

    private func storePool(_ pool: PermitPool, for context: WorkspaceContext) {
        permitPools[context] = pool.isIdle ? nil : pool
    }
}
