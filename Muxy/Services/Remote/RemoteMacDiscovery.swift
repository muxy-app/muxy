@preconcurrency import Foundation
import os

private let discoveryLogger = Logger(subsystem: "app.muxy", category: "RemoteMacDiscovery")

struct DiscoveredRemoteMac: Identifiable, Equatable {
    let id: String
    let name: String
    let host: String
    let port: UInt16
}

@MainActor
@Observable
final class RemoteMacDiscovery: NSObject {
    private(set) var devices: [DiscoveredRemoteMac] = []
    private(set) var isSearching = false

    private let browser = NetServiceBrowser()
    private var services: [String: NetService] = [:]

    override init() {
        super.init()
        browser.delegate = self
    }

    func start() {
        guard !isSearching else { return }
        isSearching = true
        browser.schedule(in: .main, forMode: .common)
        browser.searchForServices(ofType: "_muxy._tcp.", inDomain: "local.")
    }

    func stop() {
        guard isSearching else { return }
        browser.stop()
        browser.remove(from: .main, forMode: .common)
        for service in services.values {
            service.stop()
        }
        services.removeAll()
        isSearching = false
    }

    private func add(_ service: NetService) {
        services[service.name] = service
        service.delegate = self
        service.schedule(in: .main, forMode: .common)
        service.resolve(withTimeout: 5)
    }

    private func remove(_ service: NetService) {
        services.removeValue(forKey: service.name)
        devices.removeAll { $0.id == service.name }
    }

    private func resolved(_ service: NetService) {
        guard let hostName = service.hostName,
              let port = UInt16(exactly: service.port)
        else { return }
        let host = hostName.hasSuffix(".") ? String(hostName.dropLast()) : hostName
        let device = DiscoveredRemoteMac(
            id: service.name,
            name: service.name,
            host: host,
            port: port
        )
        devices.removeAll { $0.id == device.id }
        devices.append(device)
        devices.sort { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }
}

extension RemoteMacDiscovery: @preconcurrency NetServiceBrowserDelegate {
    func netServiceBrowser(
        _: NetServiceBrowser,
        didFind service: NetService,
        moreComing _: Bool
    ) {
        add(service)
    }

    func netServiceBrowser(
        _: NetServiceBrowser,
        didRemove service: NetService,
        moreComing _: Bool
    ) {
        remove(service)
    }

    func netServiceBrowser(_: NetServiceBrowser, didNotSearch error: [String: NSNumber]) {
        discoveryLogger.error("Remote Mac discovery failed: \(String(describing: error))")
        isSearching = false
    }
}

extension RemoteMacDiscovery: @preconcurrency NetServiceDelegate {
    func netServiceDidResolveAddress(_ sender: NetService) {
        resolved(sender)
    }

    func netService(_: NetService, didNotResolve errorDict: [String: NSNumber]) {
        discoveryLogger.error("Remote Mac resolution failed: \(String(describing: errorDict))")
    }
}
