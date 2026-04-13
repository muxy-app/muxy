import Testing

@testable import Muxy

@Suite("TextBackingStore", .serialized)
@MainActor
struct TextBackingStoreTests {
    @Test("initial state has one empty line")
    func initialState() {
        let store = TextBackingStore()
        #expect(store.lineCount == 1)
        #expect(store.line(at: 0) == "")
    }

    @Test("loadFromText single line")
    func loadSingleLine() {
        let store = TextBackingStore()
        store.loadFromText("hello")
        #expect(store.lineCount == 1)
        #expect(store.line(at: 0) == "hello")
    }

    @Test("loadFromText multiple lines")
    func loadMultipleLines() {
        let store = TextBackingStore()
        store.loadFromText("a\nb\nc")
        #expect(store.lineCount == 3)
        #expect(store.line(at: 0) == "a")
        #expect(store.line(at: 1) == "b")
        #expect(store.line(at: 2) == "c")
    }

    @Test("loadFromText with trailing newline")
    func loadTrailingNewline() {
        let store = TextBackingStore()
        store.loadFromText("a\nb\n")
        #expect(store.lineCount == 3)
        #expect(store.line(at: 0) == "a")
        #expect(store.line(at: 1) == "b")
        #expect(store.line(at: 2) == "")
    }

    @Test("appendText without newline appends to last line")
    func appendNoNewline() {
        let store = TextBackingStore()
        store.loadFromText("hello")
        store.appendText(" world")
        #expect(store.line(at: 0) == "hello world")
    }

    @Test("appendText with newline creates new lines")
    func appendWithNewline() {
        let store = TextBackingStore()
        store.loadFromText("first")
        store.appendText("\nsecond\nthird")
        store.finishLoading()
        #expect(store.lineCount == 3)
        #expect(store.line(at: 0) == "first")
        #expect(store.line(at: 1) == "second")
        #expect(store.line(at: 2) == "third")
    }

    @Test("appendText streaming chunks build correct lines")
    func appendMultipleChunks() {
        let store = TextBackingStore()
        store.appendText("line1\n")
        store.appendText("line2\n")
        store.appendText("line3")
        store.finishLoading()
        #expect(store.lineCount == 3)
        #expect(store.line(at: 0) == "line1")
        #expect(store.line(at: 1) == "line2")
        #expect(store.line(at: 2) == "line3")
    }

    @Test("line at valid index returns content")
    func lineAtValid() {
        let store = TextBackingStore()
        store.loadFromText("a\nb\nc")
        #expect(store.line(at: 1) == "b")
    }

    @Test("line at out-of-bounds index returns empty")
    func lineAtOutOfBounds() {
        let store = TextBackingStore()
        store.loadFromText("a")
        #expect(store.line(at: -1) == "")
        #expect(store.line(at: 5) == "")
    }

    @Test("textForRange returns joined lines")
    func textForRangeValid() {
        let store = TextBackingStore()
        store.loadFromText("a\nb\nc\nd")
        let text = store.textForRange(1 ..< 3)
        #expect(text == "b\nc")
    }

    @Test("textForRange clamps out-of-bounds")
    func textForRangeClamped() {
        let store = TextBackingStore()
        store.loadFromText("a\nb")
        let text = store.textForRange(-1 ..< 100)
        #expect(text == "a\nb")
    }

    @Test("textForRange empty range returns empty")
    func textForRangeEmpty() {
        let store = TextBackingStore()
        store.loadFromText("a\nb")
        let text = store.textForRange(1 ..< 1)
        #expect(text == "")
    }

    @Test("fullText joins all lines")
    func fullText() {
        let store = TextBackingStore()
        store.loadFromText("a\nb\nc")
        #expect(store.fullText() == "a\nb\nc")
    }

    @Test("replaceLines replaces range and returns old lines")
    func replaceLines() {
        let store = TextBackingStore()
        store.loadFromText("a\nb\nc\nd")
        let old = store.replaceLines(in: 1 ..< 3, with: ["x", "y", "z"])
        #expect(old == ["b", "c"])
        #expect(store.lineCount == 5)
        #expect(store.line(at: 1) == "x")
        #expect(store.line(at: 2) == "y")
        #expect(store.line(at: 3) == "z")
        #expect(store.line(at: 4) == "d")
    }

    @Test("search case insensitive finds matches")
    func searchCaseInsensitive() {
        let store = TextBackingStore()
        store.loadFromText("Hello World\nhello again\nno match")
        let matches = store.search(needle: "hello", caseSensitive: false, useRegex: false)
        #expect(matches.count == 2)
        #expect(matches[0].lineIndex == 0)
        #expect(matches[1].lineIndex == 1)
    }

    @Test("search case sensitive only finds exact case")
    func searchCaseSensitive() {
        let store = TextBackingStore()
        store.loadFromText("Hello World\nhello again")
        let matches = store.search(needle: "Hello", caseSensitive: true, useRegex: false)
        #expect(matches.count == 1)
        #expect(matches[0].lineIndex == 0)
    }

    @Test("search regex pattern matches")
    func searchRegex() {
        let store = TextBackingStore()
        store.loadFromText("func foo()\nvar bar = 1\nfunc baz()")
        let matches = store.search(needle: "func \\w+", caseSensitive: true, useRegex: true)
        #expect(matches.count == 2)
    }

    @Test("search invalid regex returns empty")
    func searchInvalidRegex() {
        let store = TextBackingStore()
        store.loadFromText("some text")
        let matches = store.search(needle: "[invalid", caseSensitive: true, useRegex: true)
        #expect(matches.isEmpty)
    }

    @Test("search empty needle returns empty")
    func searchEmptyNeedle() {
        let store = TextBackingStore()
        store.loadFromText("some text")
        let matches = store.search(needle: "", caseSensitive: false, useRegex: false)
        #expect(matches.isEmpty)
    }

    @Test("search finds multiple matches on same line")
    func searchMultipleOnSameLine() {
        let store = TextBackingStore()
        store.loadFromText("aaa")
        let matches = store.search(needle: "a", caseSensitive: true, useRegex: false)
        #expect(matches.count == 3)
        #expect(matches.allSatisfy { $0.lineIndex == 0 })
    }
}
