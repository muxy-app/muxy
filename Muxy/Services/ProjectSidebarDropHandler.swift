import AppKit
import Foundation
import UniformTypeIdentifiers

protocol FileURLItemProviding: AnyObject {
    func hasItemConformingToTypeIdentifier(_ typeIdentifier: String) -> Bool
    func loadItem(
        forTypeIdentifier typeIdentifier: String,
        options: [AnyHashable: Any]?,
        completionHandler: (@Sendable (NSSecureCoding?, (any Error)?) -> Void)?
    )
}

extension NSItemProvider: FileURLItemProviding {}

enum ProjectSidebarDropHandler {
    static func handle(
        providers: [any FileURLItemProviding],
        onPath: @escaping @MainActor @Sendable (String) -> ProjectOpenConfirmationResult
    ) -> Bool {
        let fileURLProviders = providers.filter { $0.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) }
        guard !fileURLProviders.isEmpty else { return false }
        process(providers: ProviderSequence(fileURLProviders), index: 0, onPath: onPath)
        return true
    }

    private static func process(
        providers: ProviderSequence,
        index: Int,
        onPath: @escaping @MainActor @Sendable (String) -> ProjectOpenConfirmationResult
    ) {
        guard index < providers.items.count else { return }
        providers.items[index].loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, _ in
            guard let path = path(from: item) else {
                process(providers: providers, index: index + 1, onPath: onPath)
                return
            }
            Task { @MainActor in
                _ = onPath(path)
                process(providers: providers, index: index + 1, onPath: onPath)
            }
        }
    }

    static func path(from item: NSSecureCoding?) -> String? {
        if let url = item as? URL, url.isFileURL {
            return url.path(percentEncoded: false)
        }
        if let data = item as? Data,
           let url = URL(dataRepresentation: data, relativeTo: nil),
           url.isFileURL
        {
            return url.path(percentEncoded: false)
        }
        return nil
    }
}

private final class ProviderSequence: @unchecked Sendable {
    let items: [any FileURLItemProviding]

    init(_ items: [any FileURLItemProviding]) {
        self.items = items
    }
}
