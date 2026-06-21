import Foundation
import SQLite3

struct ChromeCookieRow {
    let name: String
    let encryptedValue: Data
    let host: String
    let path: String
    let isSecure: Bool
    let isHTTPOnly: Bool
    let expiresUTC: Int64
}

enum ChromeCookieDatabase {
    static func readRows(at fileURL: URL) throws -> [ChromeCookieRow] {
        let copyURL = try copyToTemporary(fileURL)
        defer { try? FileManager.default.removeItem(at: copyURL) }

        var database: OpaquePointer?
        guard sqlite3_open_v2(copyURL.path, &database, SQLITE_OPEN_READONLY, nil) == SQLITE_OK,
              let database
        else {
            sqlite3_close(database)
            throw BrowserImportError.readFailed("could not open cookie database")
        }
        defer { sqlite3_close(database) }

        let sql = """
        SELECT name, encrypted_value, host_key, path, is_secure, is_httponly, expires_utc
        FROM cookies
        """
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(database, sql, -1, &statement, nil) == SQLITE_OK else {
            throw BrowserImportError.readFailed("could not query cookies table")
        }
        defer { sqlite3_finalize(statement) }

        var rows: [ChromeCookieRow] = []
        while sqlite3_step(statement) == SQLITE_ROW {
            guard let row = parseRow(statement) else { continue }
            rows.append(row)
        }
        return rows
    }

    private static func parseRow(_ statement: OpaquePointer?) -> ChromeCookieRow? {
        guard let nameCString = sqlite3_column_text(statement, 0),
              let hostCString = sqlite3_column_text(statement, 2),
              let pathCString = sqlite3_column_text(statement, 3)
        else { return nil }

        let blobLength = Int(sqlite3_column_bytes(statement, 1))
        let encryptedValue = if blobLength > 0, let blob = sqlite3_column_blob(statement, 1) {
            Data(bytes: blob, count: blobLength)
        } else {
            Data()
        }

        return ChromeCookieRow(
            name: String(cString: nameCString),
            encryptedValue: encryptedValue,
            host: String(cString: hostCString),
            path: String(cString: pathCString),
            isSecure: sqlite3_column_int(statement, 4) != 0,
            isHTTPOnly: sqlite3_column_int(statement, 5) != 0,
            expiresUTC: sqlite3_column_int64(statement, 6)
        )
    }

    private static func copyToTemporary(_ fileURL: URL) throws -> URL {
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            throw BrowserImportError.profileUnavailable
        }
        let destination = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-chrome-cookies-\(UUID().uuidString).sqlite")
        try FileManager.default.copyItem(at: fileURL, to: destination)
        return destination
    }
}
