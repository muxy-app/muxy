import Testing

@testable import Muxy

@Suite("WrappedLineHeights")
@MainActor
struct WrappedLineHeightsTests {
    @Test("baseline state has one fragment per line")
    func baseline() {
        let heights = WrappedLineHeights(lineCount: 5)
        #expect(heights.lineCount == 5)
        #expect(heights.totalFragments == 5)
        #expect(heights.fragmentCount(at: 0) == 1)
        #expect(heights.fragmentCount(at: 4) == 1)
        #expect(heights.prefixFragments(throughLine: -1) == 0)
        #expect(heights.prefixFragments(throughLine: 4) == 5)
    }

    @Test("setFragmentCount updates totals and prefix sums")
    func setFragmentCount() {
        let heights = WrappedLineHeights(lineCount: 4)
        heights.setFragmentCount(3, at: 1)
        heights.setFragmentCount(2, at: 3)
        #expect(heights.totalFragments == 7)
        #expect(heights.fragmentCount(at: 1) == 3)
        #expect(heights.prefixFragments(throughLine: 0) == 1)
        #expect(heights.prefixFragments(throughLine: 1) == 4)
        #expect(heights.prefixFragments(throughLine: 2) == 5)
        #expect(heights.prefixFragments(throughLine: 3) == 7)
    }

    @Test("setFragmentCount clamps to minimum 1")
    func clampsMinimum() {
        let heights = WrappedLineHeights(lineCount: 3)
        heights.setFragmentCount(0, at: 1)
        heights.setFragmentCount(-5, at: 2)
        #expect(heights.fragmentCount(at: 1) == 1)
        #expect(heights.fragmentCount(at: 2) == 1)
        #expect(heights.totalFragments == 3)
    }

    @Test("setFragmentCount ignores out-of-range indices")
    func outOfRange() {
        let heights = WrappedLineHeights(lineCount: 2)
        heights.setFragmentCount(5, at: 5)
        heights.setFragmentCount(5, at: -1)
        #expect(heights.totalFragments == 2)
        #expect(heights.fragmentCount(at: 99) == 1)
    }

    @Test("line(forFragmentOffset:) maps fragment offsets back to source lines")
    func fragmentOffsetMapping() {
        let heights = WrappedLineHeights(lineCount: 3)
        heights.setFragmentCount(2, at: 0)
        heights.setFragmentCount(3, at: 1)
        #expect(heights.line(forFragmentOffset: 0) == 0)
        #expect(heights.line(forFragmentOffset: 1) == 0)
        #expect(heights.line(forFragmentOffset: 2) == 1)
        #expect(heights.line(forFragmentOffset: 4) == 1)
        #expect(heights.line(forFragmentOffset: 5) == 2)
        #expect(heights.line(forFragmentOffset: 999) == 2)
    }

    @Test("resetAllToBaseline restores one fragment per line")
    func resetAll() {
        let heights = WrappedLineHeights(lineCount: 3)
        heights.setFragmentCount(4, at: 0)
        heights.setFragmentCount(2, at: 2)
        heights.resetAllToBaseline()
        #expect(heights.totalFragments == 3)
        #expect(heights.fragmentCount(at: 0) == 1)
        #expect(heights.prefixFragments(throughLine: 2) == 3)
    }

    @Test("resize to larger count fills with baseline")
    func resizeLarger() {
        let heights = WrappedLineHeights(lineCount: 2)
        heights.setFragmentCount(5, at: 0)
        heights.resize(to: 4)
        #expect(heights.lineCount == 4)
        #expect(heights.totalFragments == 4)
        #expect(heights.fragmentCount(at: 0) == 1)
    }

    @Test("resize to same count is a no-op")
    func resizeSame() {
        let heights = WrappedLineHeights(lineCount: 3)
        heights.setFragmentCount(4, at: 1)
        heights.resize(to: 3)
        #expect(heights.totalFragments == 6)
        #expect(heights.fragmentCount(at: 1) == 4)
    }

    @Test("replaceLines with equal counts resets affected lines to baseline")
    func replaceEqual() {
        let heights = WrappedLineHeights(lineCount: 5)
        heights.setFragmentCount(3, at: 1)
        heights.setFragmentCount(4, at: 2)
        heights.replaceLines(start: 1, removingCount: 2, insertingCount: 2)
        #expect(heights.lineCount == 5)
        #expect(heights.fragmentCount(at: 1) == 1)
        #expect(heights.fragmentCount(at: 2) == 1)
        #expect(heights.totalFragments == 5)
    }

    @Test("replaceLines grows when inserting more than removing")
    func replaceGrow() {
        let heights = WrappedLineHeights(lineCount: 3)
        heights.setFragmentCount(2, at: 0)
        heights.replaceLines(start: 1, removingCount: 1, insertingCount: 3)
        #expect(heights.lineCount == 5)
        #expect(heights.fragmentCount(at: 0) == 2)
        #expect(heights.totalFragments == 6)
        #expect(heights.prefixFragments(throughLine: 0) == 2)
    }

    @Test("replaceLines shrinks when removing more than inserting")
    func replaceShrink() {
        let heights = WrappedLineHeights(lineCount: 5)
        heights.setFragmentCount(3, at: 0)
        heights.replaceLines(start: 1, removingCount: 3, insertingCount: 0)
        #expect(heights.lineCount == 2)
        #expect(heights.fragmentCount(at: 0) == 3)
        #expect(heights.totalFragments == 4)
    }

    @Test("empty store is safe to query")
    func empty() {
        let heights = WrappedLineHeights(lineCount: 0)
        #expect(heights.lineCount == 0)
        #expect(heights.totalFragments == 0)
        #expect(heights.line(forFragmentOffset: 0) == 0)
        #expect(heights.prefixFragments(throughLine: 0) == 0)
    }
}
