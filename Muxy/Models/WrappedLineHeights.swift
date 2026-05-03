import Foundation

@MainActor
final class WrappedLineHeights {
    private var fragments: [Int]
    private var bit: [Int]
    private(set) var totalFragments: Int

    init(lineCount: Int) {
        let count = max(0, lineCount)
        fragments = Array(repeating: 1, count: count)
        bit = Array(repeating: 0, count: count + 1)
        totalFragments = count
        for index in 0 ..< count {
            bitAdd(index + 1, delta: 1)
        }
    }

    var lineCount: Int { fragments.count }

    func fragmentCount(at line: Int) -> Int {
        guard line >= 0, line < fragments.count else { return 1 }
        return fragments[line]
    }

    func setFragmentCount(_ count: Int, at line: Int) {
        guard line >= 0, line < fragments.count else { return }
        let normalized = max(1, count)
        let delta = normalized - fragments[line]
        guard delta != 0 else { return }
        fragments[line] = normalized
        totalFragments += delta
        bitAdd(line + 1, delta: delta)
    }

    func resetAllToBaseline() {
        let count = fragments.count
        fragments = Array(repeating: 1, count: count)
        bit = Array(repeating: 0, count: count + 1)
        totalFragments = count
        for index in 0 ..< count {
            bitAdd(index + 1, delta: 1)
        }
    }

    func resize(to lineCount: Int) {
        let target = max(0, lineCount)
        if target == fragments.count { return }
        fragments = Array(repeating: 1, count: target)
        bit = Array(repeating: 0, count: target + 1)
        totalFragments = target
        for index in 0 ..< target {
            bitAdd(index + 1, delta: 1)
        }
    }

    func replaceLines(start: Int, removingCount: Int, insertingCount: Int) {
        let safeStart = max(0, min(start, fragments.count))
        let safeRemove = max(0, min(removingCount, fragments.count - safeStart))
        let safeInsert = max(0, insertingCount)
        if safeRemove == safeInsert {
            for offset in 0 ..< safeRemove {
                setFragmentCount(1, at: safeStart + offset)
            }
            return
        }
        let endRemoved = safeStart + safeRemove
        let insertedSlice = Array(repeating: 1, count: safeInsert)
        fragments.replaceSubrange(safeStart ..< endRemoved, with: insertedSlice)
        let newCount = fragments.count
        bit = Array(repeating: 0, count: newCount + 1)
        totalFragments = 0
        for index in 0 ..< newCount {
            totalFragments += fragments[index]
            bitAdd(index + 1, delta: fragments[index])
        }
    }

    func prefixFragments(throughLine line: Int) -> Int {
        let clamped = min(max(0, line + 1), fragments.count)
        return bitPrefix(clamped)
    }

    func line(forFragmentOffset offset: Int) -> Int {
        guard !fragments.isEmpty else { return 0 }
        let target = max(1, min(offset + 1, totalFragments))
        var index = 0
        var remaining = target
        var step = 1
        while step * 2 <= bit.count - 1 {
            step *= 2
        }
        while step > 0 {
            let next = index + step
            if next < bit.count, bit[next] < remaining {
                index = next
                remaining -= bit[next]
            }
            step /= 2
        }
        return min(fragments.count - 1, index)
    }

    private func bitAdd(_ index: Int, delta: Int) {
        var i = index
        while i < bit.count {
            bit[i] += delta
            i += i & -i
        }
    }

    private func bitPrefix(_ index: Int) -> Int {
        var sum = 0
        var i = index
        while i > 0 {
            sum += bit[i]
            i -= i & -i
        }
        return sum
    }
}
