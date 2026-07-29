import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionLocalizationCatalog")
struct ExtensionLocalizationCatalogTests {
    @Test("accepts translations that preserve placeholders")
    func acceptsPreservedPlaceholders() {
        let catalog: [String: Any] = [
            "Settings": "Einstellungen",
            "Created branch %@": "Zweig %@ erstellt",
            "%lld changes": "%lld Änderungen",
            "%lld%%": "%lld%%",
            "Deleted %@ at %@": "%@ um %@ gelöscht",
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("accepts reordering through positional placeholders")
    func acceptsPositionalReordering() {
        let catalog: [String: Any] = ["%@ (%@)": "%2$@ – %1$@"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("accepts translations that drop a placeholder")
    func acceptsDroppedPlaceholder() {
        let catalog: [String: Any] = ["%@ (%@)": "%1$@"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("accepts equivalent 64-bit length modifiers")
    func acceptsEquivalentLengthModifiers() {
        let catalog: [String: Any] = ["%lld changes": "%ld Änderungen"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("accepts flags, width and precision")
    func acceptsFlagsWidthAndPrecision() {
        let catalog: [String: Any] = ["%lld%%": "%+08lld%%"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("rejects an added placeholder")
    func rejectsAddedPlaceholder() {
        let catalog: [String: Any] = ["Created branch %@": "Zweig %@ %@ erstellt"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "Created branch %@")
    }

    @Test("rejects a placeholder added to a key that has none")
    func rejectsPlaceholderOnLiteralKey() {
        let catalog: [String: Any] = ["Settings": "Einstellungen %@"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "Settings")
    }

    @Test("rejects object-to-integer type confusion")
    func rejectsTypeConfusion() {
        let catalog: [String: Any] = ["%lld changes": "%@ Änderungen"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "%lld changes")
    }

    @Test("rejects C string and pointer conversions")
    func rejectsUnsafeConversions() {
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["Created branch %@": "Zweig %s erstellt"])
            == "Created branch %@")
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["Created branch %@": "Zweig %p erstellt"])
            == "Created branch %@")
    }

    @Test("rejects out-of-range positional placeholders")
    func rejectsOutOfRangePosition() {
        let catalog: [String: Any] = ["Created branch %@": "Zweig %2$@ erstellt"]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "Created branch %@")
    }

    @Test("rejects argument-consuming width and precision")
    func rejectsStarWidth() {
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["%lld changes": "%*lld Änderungen"])
            == "%lld changes")
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["%lld changes": "%.*lld Änderungen"])
            == "%lld changes")
    }

    @Test("rejects truncated and unknown conversions")
    func rejectsMalformedConversions() {
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["Settings": "Einstellungen %"]) == "Settings")
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["Created branch %@": "Zweig %n erstellt"])
            == "Created branch %@")
    }

    @Test("rejects non-string catalog values")
    func rejectsNonStringValues() {
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: ["Settings": 42]) == "Settings")
    }

    @Test("accepts a plural entry that matches the key placeholder")
    func acceptsMatchingPluralEntry() {
        let catalog: [String: Any] = [
            "%lld changes": [
                ExtensionLocalizationCatalog.pluralFormatKey: "%#@count@",
                "count": [
                    ExtensionLocalizationCatalog.pluralSpecTypeKey: "NSStringPluralRuleType",
                    ExtensionLocalizationCatalog.pluralValueTypeKey: "lld",
                    "one": "%lld Änderung",
                    "other": "%lld Änderungen",
                ],
            ],
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("accepts a plural entry that positions its variable after a literal placeholder")
    func acceptsPluralEntryAfterLiteralPlaceholder() {
        let catalog: [String: Any] = [
            "%@ has %lld changes": [
                ExtensionLocalizationCatalog.pluralFormatKey: "%@ hat %#@count@",
                "count": [
                    ExtensionLocalizationCatalog.pluralSpecTypeKey: "NSStringPluralRuleType",
                    ExtensionLocalizationCatalog.pluralValueTypeKey: "lld",
                    "one": "%lld Änderung",
                    "other": "%lld Änderungen",
                ],
            ],
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == nil)
    }

    @Test("rejects a plural variant that adds a placeholder")
    func rejectsPluralVariantWithExtraPlaceholder() {
        let catalog: [String: Any] = [
            "%lld changes": [
                ExtensionLocalizationCatalog.pluralFormatKey: "%#@count@",
                "count": [
                    ExtensionLocalizationCatalog.pluralSpecTypeKey: "NSStringPluralRuleType",
                    ExtensionLocalizationCatalog.pluralValueTypeKey: "lld",
                    "one": "%lld Änderung",
                    "other": "%lld %@ Änderungen",
                ],
            ],
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "%lld changes")
    }

    @Test("rejects a plural entry whose format key adds a placeholder")
    func rejectsPluralFormatKeyWithExtraPlaceholder() {
        let catalog: [String: Any] = [
            "%lld changes": [
                ExtensionLocalizationCatalog.pluralFormatKey: "%#@count@ %@",
                "count": [
                    ExtensionLocalizationCatalog.pluralSpecTypeKey: "NSStringPluralRuleType",
                    ExtensionLocalizationCatalog.pluralValueTypeKey: "lld",
                    "one": "%lld Änderung",
                    "other": "%lld Änderungen",
                ],
            ],
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "%lld changes")
    }

    @Test("rejects a plural entry with a missing variable or format key")
    func rejectsIncompletePluralEntry() {
        let missingVariable: [String: Any] = [
            "%lld changes": [ExtensionLocalizationCatalog.pluralFormatKey: "%#@count@"],
        ]
        let missingFormatKey: [String: Any] = [
            "%lld changes": ["count": ["other": "%lld Änderungen"]],
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: missingVariable) == "%lld changes")
        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: missingFormatKey) == "%lld changes")
    }

    @Test("reports the offending key deterministically")
    func reportsOffendingKeyDeterministically() {
        let catalog: [String: Any] = [
            "alpha %@": "alpha %@",
            "beta %@": "beta %@ %@",
            "gamma %@": "gamma %@ %@",
        ]

        #expect(ExtensionLocalizationCatalog.incompatibleKey(in: catalog) == "beta %@")
    }
}
