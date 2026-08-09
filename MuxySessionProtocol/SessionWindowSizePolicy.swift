public enum SessionWindowSizePolicy {
    public static let fallbackColumns: UInt16 = 80
    public static let fallbackRows: UInt16 = 24
    public static let minimumColumns: UInt16 = 10
    public static let minimumRows: UInt16 = 4

    public static func isUsable(columns: UInt16, rows: UInt16) -> Bool {
        columns >= minimumColumns && rows >= minimumRows
    }

    public static func attachSize(from size: (columns: UInt16, rows: UInt16)?) -> (columns: UInt16, rows: UInt16) {
        guard let size, isUsable(columns: size.columns, rows: size.rows) else {
            return (0, 0)
        }
        return size
    }

    public static func createSize(columns: UInt16, rows: UInt16) -> (columns: UInt16, rows: UInt16) {
        guard isUsable(columns: columns, rows: rows) else {
            return (fallbackColumns, fallbackRows)
        }
        return (columns, rows)
    }
}
