import Foundation

struct StyleOverride: Identifiable, Equatable {
    enum Property: String, CaseIterable, Identifiable, Equatable {
        case fontFamily
        case fontSize
        case fontWeight
        case color
        case backgroundColor
        case paddingTop
        case paddingRight
        case paddingBottom
        case paddingLeft
        case marginTop
        case marginRight
        case marginBottom
        case marginLeft
        case borderRadius

        var id: String { rawValue }

        var cssName: String {
            switch self {
            case .fontFamily: "font-family"
            case .fontSize: "font-size"
            case .fontWeight: "font-weight"
            case .color: "color"
            case .backgroundColor: "background-color"
            case .paddingTop: "padding-top"
            case .paddingRight: "padding-right"
            case .paddingBottom: "padding-bottom"
            case .paddingLeft: "padding-left"
            case .marginTop: "margin-top"
            case .marginRight: "margin-right"
            case .marginBottom: "margin-bottom"
            case .marginLeft: "margin-left"
            case .borderRadius: "border-radius"
            }
        }

        var displayName: String {
            switch self {
            case .fontFamily: "Font Family"
            case .fontSize: "Font Size"
            case .fontWeight: "Font Weight"
            case .color: "Color"
            case .backgroundColor: "Background"
            case .paddingTop: "Padding Top"
            case .paddingRight: "Padding Right"
            case .paddingBottom: "Padding Bottom"
            case .paddingLeft: "Padding Left"
            case .marginTop: "Margin Top"
            case .marginRight: "Margin Right"
            case .marginBottom: "Margin Bottom"
            case .marginLeft: "Margin Left"
            case .borderRadius: "Border Radius"
            }
        }
    }

    enum Scope: String, Equatable {
        case element
        case allMatching
    }

    let id: UUID
    var selector: String
    var property: Property
    var originalValue: String
    var value: String
    var scope: Scope

    init(
        id: UUID = UUID(),
        selector: String,
        property: Property,
        originalValue: String,
        value: String,
        scope: Scope = .element
    ) {
        self.id = id
        self.selector = selector
        self.property = property
        self.originalValue = originalValue
        self.value = value
        self.scope = scope
    }
}
