import Foundation
import SwiftUI

@MainActor
@Observable
final class LocalizationService {
    static let shared = LocalizationService()

    private(set) var activeSelection = LocalizationSelection.builtinValue
    private(set) var locale = Locale(identifier: "en")
    private(set) var bundleURL: URL?

    private let defaults: UserDefaults
    private var searchStringCache: [String: String] = [:]

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func select(_ value: String, store: ExtensionStore = .shared) {
        defaults.set(value, forKey: LocalizationSelection.storageKey)
        refresh(store: store)
    }

    func refresh(store: ExtensionStore = .shared) {
        let storedValue = defaults.string(forKey: LocalizationSelection.storageKey)
            ?? LocalizationSelection.builtinValue
        refresh(storedValue: storedValue, bindings: store.localizations())
    }

    func refresh(
        storedValue: String,
        bindings: [ExtensionStore.LocalizationBinding]
    ) {
        searchStringCache.removeAll(keepingCapacity: true)
        let previousSelection = activeSelection
        let previousLocale = locale
        let previousBundleURL = bundleURL
        guard let identifier = LocalizationSelection.parse(storedValue),
              let binding = bindings.first(where: {
                  $0.muxyExtension.id == identifier.extensionID
                      && $0.localization.id == identifier.localizationID
              }),
              let resolvedBundleURL = binding.bundleURL
        else {
            activeSelection = LocalizationSelection.builtinValue
            locale = Locale(identifier: "en")
            bundleURL = nil
            postChangeIfNeeded(
                previousSelection: previousSelection,
                previousLocale: previousLocale,
                previousBundleURL: previousBundleURL
            )
            return
        }
        activeSelection = binding.id
        locale = Locale(identifier: binding.localization.language)
        bundleURL = resolvedBundleURL
        postChangeIfNeeded(
            previousSelection: previousSelection,
            previousLocale: previousLocale,
            previousBundleURL: previousBundleURL
        )
    }

    func resource(_ resource: LocalizedStringResource) -> LocalizedStringResource {
        var localizedResource = resource
        localizedResource.locale = locale
        guard let bundleURL else { return localizedResource }
        return LocalizedStringResource(
            resource.defaultValue,
            table: resource.table,
            locale: locale,
            bundle: .atURL(bundleURL)
        )
    }

    func string(_ resource: LocalizedStringResource) -> String {
        String(localized: self.resource(resource))
    }

    func searchString(key: String) -> String {
        if let cached = searchStringCache[key] {
            return cached
        }
        let value = string(LocalizedStringResource(String.LocalizationValue(key)))
        searchStringCache[key] = value
        return value
    }

    private func postChangeIfNeeded(
        previousSelection: String,
        previousLocale: Locale,
        previousBundleURL: URL?
    ) {
        guard previousSelection != activeSelection
            || previousLocale.identifier != locale.identifier
            || previousBundleURL != bundleURL
        else { return }
        NotificationCenter.default.post(name: .localizationDidChange, object: self)
    }
}

@MainActor
enum LocalizedSearch {
    static func matches(
        query: String,
        localizedKeys: [String] = [],
        verbatimValues: [String] = [],
        localization: LocalizationService = .shared
    ) -> Bool {
        let normalizedQuery = normalize(query, locale: localization.locale)
        guard !normalizedQuery.isEmpty else { return true }
        let localizedValues = localizedKeys.flatMap { key in
            [key, localization.searchString(key: key)]
        }
        let searchableText = (localizedValues + verbatimValues)
            .map { normalize($0, locale: localization.locale) }
            .joined(separator: " ")
        return searchableText.contains(normalizedQuery)
    }

    private static func normalize(_ value: String, locale: Locale) -> String {
        value
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .folding(
                options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive],
                locale: locale
            )
    }
}

@MainActor
enum L10n {
    static func resource(_ resource: LocalizedStringResource) -> LocalizedStringResource {
        LocalizationService.shared.resource(resource)
    }

    static func resource(key: String) -> LocalizedStringResource {
        resource(LocalizedStringResource(String.LocalizationValue(key)))
    }

    static func string(_ resource: LocalizedStringResource) -> String {
        LocalizationService.shared.string(resource)
    }

    static func string(key: String) -> String {
        string(LocalizedStringResource(String.LocalizationValue(key)))
    }
}

struct LocalizationEnvironment<Content: View>: View {
    @State private var localizationService = LocalizationService.shared
    @ViewBuilder let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .environment(localizationService)
            .environment(\.locale, localizationService.locale)
    }
}
