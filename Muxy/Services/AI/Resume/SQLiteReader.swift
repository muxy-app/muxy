import Foundation
import SQLite3

enum SQLiteReader {
    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

    static func rows(databasePath: String, query: String, parameters: [String]) -> [[String: String]] {
        var database: OpaquePointer?
        guard sqlite3_open_v2(databasePath, &database, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else {
            sqlite3_close(database)
            return []
        }
        defer { sqlite3_close(database) }

        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, query, -1, &statement, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(statement) }

        for (index, value) in parameters.enumerated() {
            sqlite3_bind_text(statement, Int32(index + 1), value, -1, transient)
        }

        var results: [[String: String]] = []
        while sqlite3_step(statement) == SQLITE_ROW {
            var row: [String: String] = [:]
            for column in 0 ..< sqlite3_column_count(statement) {
                guard let name = sqlite3_column_name(statement, column) else { continue }
                if let text = sqlite3_column_text(statement, column) {
                    row[String(cString: name)] = String(cString: text)
                }
            }
            results.append(row)
        }
        return results
    }
}
