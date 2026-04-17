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

    private(set) var pendingRequest: PairingRequest?

    private var continuations: [UUID: CheckedContinuation<Bool, Never>] = [:]
    private var queue: [PairingRequest] = []

    private init() {}

    func requestApproval(deviceID: UUID, deviceName: String, token: String) async -> Bool {
        let request = PairingRequest(
            deviceID: deviceID,
            deviceName: deviceName,
            token: token,
            receivedAt: Date()
        )
        return await withCheckedContinuation { continuation in
            continuations[request.id] = continuation
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
        finish(request, approved: true)
    }

    func deny(_ request: PairingRequest) {
        finish(request, approved: false)
    }

    private func finish(_ request: PairingRequest, approved: Bool) {
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
        NSApp.activate(ignoringOtherApps: true)
    }
}
