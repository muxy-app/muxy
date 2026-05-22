import Foundation

struct BrowserAnnotation: Identifiable, Equatable {
    enum Status: String, Equatable {
        case draft
        case sent
    }

    let id: UUID
    var selector: String
    var xpath: String
    var textSnippet: String
    var rect: CGRect
    var pageURL: String
    var pageTitle: String
    var viewportWidth: CGFloat
    var viewportHeight: CGFloat
    var comment: String
    var styleOverrides: [StyleOverride]
    var screenshotPNG: Data?
    var createdAt: Date
    var status: Status

    init(
        id: UUID = UUID(),
        selector: String,
        xpath: String,
        textSnippet: String,
        rect: CGRect,
        pageURL: String,
        pageTitle: String,
        viewportWidth: CGFloat,
        viewportHeight: CGFloat,
        comment: String = "",
        styleOverrides: [StyleOverride] = [],
        screenshotPNG: Data? = nil,
        createdAt: Date = Date(),
        status: Status = .draft
    ) {
        self.id = id
        self.selector = selector
        self.xpath = xpath
        self.textSnippet = textSnippet
        self.rect = rect
        self.pageURL = pageURL
        self.pageTitle = pageTitle
        self.viewportWidth = viewportWidth
        self.viewportHeight = viewportHeight
        self.comment = comment
        self.styleOverrides = styleOverrides
        self.screenshotPNG = screenshotPNG
        self.createdAt = createdAt
        self.status = status
    }
}
