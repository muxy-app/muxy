import AppKit
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "PairingRequestCoordinator")

struct PairingRequest: Identifiable, Equatable {
    let id = UUID()
    let deviceID: UUID
    let deviceName: String
    let token: String
    let receivedAt: Date
}

@MainActor
@Observable
final class PairingRequestCoordinator {
    static let shared = PairingRequestCoordinator()

    nonisolated static let tokenLengthRange: ClosedRange<Int> = 8 ... 256
    nonisolated static let deviceNameLengthRange: ClosedRange<Int> = 1 ... 128
    static let maxPendingQueue: Int = 4
    static let denialCooldownSeconds: TimeInterval = 30
    static let pairingTimeoutSeconds: TimeInterval = 60

    private(set) var pendingRequest: PairingRequest?

    private var continuations: [UUID: CheckedContinuation<Bool, Never>] = [:]
    private var queue: [PairingRequest] = []
    private var deniedAt: [UUID: Date] = [:]
    private var timeoutTasks: [UUID: Task<Void, Never>] = [:]

    private init() {}

    nonisolated static func isValidRequest(deviceName: String, token: String) -> Bool {
        deviceNameLengthRange.contains(deviceName.count) && tokenLengthRange.contains(token.count)
    }

    func requestApproval(deviceID: UUID, deviceName: String, token: String) async -> Bool {
        guard Self.isValidRequest(deviceName: deviceName, token: token) else {
            logger.warning("Rejected pairing request: invalid payload length deviceID=\(deviceID)")
            return false
        }
        if let lastDenial = deniedAt[deviceID],
           Date().timeIntervalSince(lastDenial) < Self.denialCooldownSeconds
        {
            logger.info("Rejected pairing request: cooldown deviceID=\(deviceID)")
            return false
        }
        if pendingRequest?.deviceID == deviceID || queue.contains(where: { $0.deviceID == deviceID }) {
            logger.info("Rejected pairing request: duplicate deviceID=\(deviceID)")
            return false
        }
        if pendingRequest != nil, queue.count >= Self.maxPendingQueue {
            logger.warning("Rejected pairing request: queue full deviceID=\(deviceID)")
            return false
        }

        let request = PairingRequest(
            deviceID: deviceID,
            deviceName: deviceName,
            token: token,
            receivedAt: Date()
        )
        return await withCheckedContinuation { continuation in
            continuations[request.id] = continuation
            scheduleTimeout(for: request)
            if pendingRequest == nil {
                present(request)
            } else {
                queue.append(request)
            }
        }
    }

    func approve(_ request: PairingRequest) {
        ApprovedDevicesStore.shared.approve(
            deviceID: request.deviceID,
            name: request.deviceName,
            token: request.token
        )
        finish(request, approved: true, recordDenial: false)
    }

    func deny(_ request: PairingRequest) {
        finish(request, approved: false, recordDenial: true)
    }

    private func finish(_ request: PairingRequest, approved: Bool, recordDenial: Bool) {
        timeoutTasks[request.id]?.cancel()
        timeoutTasks.removeValue(forKey: request.id)
        if recordDenial {
            deniedAt[request.deviceID] = Date()
            pruneStaleDenials()
        }
        guard let continuation = continuations.removeValue(forKey: request.id) else { return }
        continuation.resume(returning: approved)
        if pendingRequest?.id == request.id {
            pendingRequest = nil
            if let next = queue.first {
                queue.removeFirst()
                present(next)
            }
        } else {
            queue.removeAll { $0.id == request.id }
        }
    }

    private func present(_ request: PairingRequest) {
        pendingRequest = request
        if !hasRecentDenial() {
            NSApp.activate(ignoringOtherApps: true)
        }
        DispatchQueue.main.async { [weak self] in
            self?.runAlert(for: request)
        }
    }

    private func hasRecentDenial() -> Bool {
        let now = Date()
        return deniedAt.values.contains { now.timeIntervalSince($0) < Self.denialCooldownSeconds }
    }

    private func pruneStaleDenials() {
        let now = Date()
        deniedAt = deniedAt.filter { now.timeIntervalSince($0.value) < Self.denialCooldownSeconds * 2 }
    }

    private func scheduleTimeout(for request: PairingRequest) {
        let task = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(Self.pairingTimeoutSeconds))
            guard !Task.isCancelled, let self else { return }
            self.handleTimeout(for: request)
        }
        timeoutTasks[request.id] = task
    }

    private func handleTimeout(for request: PairingRequest) {
        let isPending = pendingRequest?.id == request.id
        let inQueue = queue.contains { $0.id == request.id }
        guard isPending || inQueue else { return }
        logger.info("Pairing request timed out deviceID=\(request.deviceID)")
        if isPending {
            NSApp.abortModal()
        }
        finish(request, approved: false, recordDenial: true)
    }

    private func runAlert(for request: PairingRequest) {
        guard pendingRequest?.id == request.id else { return }

        let alert = NSAlert()
        alert.messageText = "Allow \(request.deviceName) to connect?"
        alert.informativeText = "This device is requesting access to Muxy. Only approve devices you recognize."
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Approve")
        alert.addButton(withTitle: "Deny")
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"

        let response = alert.runModal()
        guard pendingRequest?.id == request.id else { return }

        if response == .alertFirstButtonReturn {
            approve(request)
        } else {
            deny(request)
        }
    }
}
