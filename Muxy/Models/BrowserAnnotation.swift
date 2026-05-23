import Foundation

struct BrowserAnnotation: Identifiable, Equatable {
    enum Status: String, Equatable {
        case draft
        case sent
    }

    let id: UUID
    var selector: String
    var selectorMinimal: String
    var xpath: String
    var textSnippet: String
    var outerHTML: String
    var rect: CGRect
    var pageURL: String
    var pageTitle: String
    var viewportWidth: CGFloat
    var viewportHeight: CGFloat
    var documentDir: String
    var documentLang: String
    var stylesheets: [String]
    var computedStyle: [String: String]
    var comment: String
    var styleOverrides: [StyleOverride]
    var screenshotURL: URL?
    var createdAt: Date
    var status: Status

    init(
        id: UUID = UUID(),
        selector: String,
        selectorMinimal: String = "",
        xpath: String,
        textSnippet: String,
        outerHTML: String = "",
        rect: CGRect,
        pageURL: String,
        pageTitle: String,
        viewportWidth: CGFloat,
        viewportHeight: CGFloat,
        documentDir: String = "",
        documentLang: String = "",
        stylesheets: [String] = [],
        computedStyle: [String: String] = [:],
        comment: String = "",
        styleOverrides: [StyleOverride] = [],
        screenshotURL: URL? = nil,
        createdAt: Date = Date(),
        status: Status = .draft
    ) {
        self.id = id
        self.selector = selector
        self.selectorMinimal = selectorMinimal
        self.xpath = xpath
        self.textSnippet = textSnippet
        self.outerHTML = outerHTML
        self.rect = rect
        self.pageURL = pageURL
        self.pageTitle = pageTitle
        self.viewportWidth = viewportWidth
        self.viewportHeight = viewportHeight
        self.documentDir = documentDir
        self.documentLang = documentLang
        self.stylesheets = stylesheets
        self.computedStyle = computedStyle
        self.comment = comment
        self.styleOverrides = styleOverrides
        self.screenshotURL = screenshotURL
        self.createdAt = createdAt
        self.status = status
    }
}
