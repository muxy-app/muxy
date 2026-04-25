import Foundation

enum GitPRParser {
    static func parsePRInfo(_ json: String) -> GitRepositoryService.PRInfo? {
        guard let data = json.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return parsePRInfo(object)
    }

    static func parsePRInfo(_ json: [String: Any]) -> GitRepositoryService.PRInfo? {
        guard let url = json["url"] as? String,
              let number = json["number"] as? Int,
              let stateRaw = json["state"] as? String
        else { return nil }

        let state = GitRepositoryService.PRState(rawValue: stateRaw) ?? .open
        let isDraft = json["isDraft"] as? Bool ?? false
        let baseBranch = json["baseRefName"] as? String ?? ""
        let mergeable: Bool? = switch json["mergeable"] as? String {
        case "MERGEABLE": true
        case "CONFLICTING": false
        default: nil
        }
        let mergeStateStatus = GitRepositoryService.PRMergeStateStatus(
            rawValue: (json["mergeStateStatus"] as? String) ?? ""
        ) ?? .unknown
        let rollup = json["statusCheckRollup"] as? [[String: Any]] ?? []
        let isCrossRepository = json["isCrossRepository"] as? Bool ?? false

        return GitRepositoryService.PRInfo(
            url: url,
            number: number,
            state: state,
            isDraft: isDraft,
            baseBranch: baseBranch,
            mergeable: mergeable,
            mergeStateStatus: mergeStateStatus,
            checks: parseStatusChecks(rollup),
            isCrossRepository: isCrossRepository
        )
    }

    static func parseStatusChecks(_ rollup: [[String: Any]]) -> GitRepositoryService.PRChecks {
        if rollup.isEmpty {
            return GitRepositoryService.PRChecks(status: .none, passing: 0, failing: 0, pending: 0, total: 0)
        }

        var passing = 0
        var failing = 0
        var pending = 0

        for entry in rollup {
            switch classifyOutcome(entry) {
            case .passing: passing += 1
            case .failing: failing += 1
            case .pending: pending += 1
            }
        }

        let total = passing + failing + pending
        let status: GitRepositoryService.PRChecksStatus = if failing > 0 {
            .failure
        } else if pending > 0 {
            .pending
        } else if passing > 0 {
            .success
        } else {
            .none
        }
        return GitRepositoryService.PRChecks(
            status: status,
            passing: passing,
            failing: failing,
            pending: pending,
            total: total
        )
    }

    static func parsePRInfoMatchingHeadSha(_ json: String, headSha: String) -> GitRepositoryService.PRInfo? {
        guard let data = json.data(using: .utf8),
              let array = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return nil }
        let normalized = headSha.lowercased()
        let match = array.first { entry in
            (entry["headRefOid"] as? String)?.lowercased() == normalized
        }
        guard let match else { return nil }
        return parsePRInfo(match)
    }

    static func parsePRList(_ json: String) -> [GitRepositoryService.PRListItem] {
        guard let data = json.data(using: .utf8),
              let array = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }

        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let fallbackFormatter = ISO8601DateFormatter()

        return array.compactMap { entry -> GitRepositoryService.PRListItem? in
            guard let number = entry["number"] as? Int,
                  let title = entry["title"] as? String
            else { return nil }
            let author = (entry["author"] as? [String: Any])?["login"] as? String ?? ""
            let headBranch = entry["headRefName"] as? String ?? ""
            let headRefOid = entry["headRefOid"] as? String ?? ""
            let baseBranch = entry["baseRefName"] as? String ?? ""
            let stateRaw = (entry["state"] as? String) ?? "OPEN"
            let state = GitRepositoryService.PRState(rawValue: stateRaw) ?? .open
            let isDraft = entry["isDraft"] as? Bool ?? false
            let url = entry["url"] as? String ?? ""
            var updatedAt: Date?
            if let raw = entry["updatedAt"] as? String {
                updatedAt = formatter.date(from: raw) ?? fallbackFormatter.date(from: raw)
            }
            let rollup = entry["statusCheckRollup"] as? [[String: Any]] ?? []
            let mergeable: Bool? = switch entry["mergeable"] as? String {
            case "MERGEABLE": true
            case "CONFLICTING": false
            default: nil
            }
            let mergeStateStatus = GitRepositoryService.PRMergeStateStatus(
                rawValue: (entry["mergeStateStatus"] as? String) ?? ""
            ) ?? .unknown
            return GitRepositoryService.PRListItem(
                number: number,
                title: title,
                author: author,
                headBranch: headBranch,
                headRefOid: headRefOid,
                baseBranch: baseBranch,
                state: state,
                isDraft: isDraft,
                url: url,
                updatedAt: updatedAt,
                checks: parseStatusChecks(rollup),
                mergeable: mergeable,
                mergeStateStatus: mergeStateStatus
            )
        }
    }

    static func parseAheadBehind(counts: String, hasUpstream: Bool) -> GitRepositoryService.AheadBehind {
        guard hasUpstream else {
            return GitRepositoryService.AheadBehind(ahead: 0, behind: 0, hasUpstream: false)
        }
        let parts = counts
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .split(whereSeparator: { $0 == "\t" || $0 == " " })
        guard parts.count == 2,
              let ahead = Int(parts[0]),
              let behind = Int(parts[1])
        else {
            return GitRepositoryService.AheadBehind(ahead: 0, behind: 0, hasUpstream: true)
        }
        return GitRepositoryService.AheadBehind(ahead: ahead, behind: behind, hasUpstream: true)
    }

    private enum Outcome {
        case passing
        case failing
        case pending
    }

    private static func classifyOutcome(_ entry: [String: Any]) -> Outcome {
        let typename = entry["__typename"] as? String ?? ""
        let raw: String = if typename == "CheckRun" {
            if (entry["status"] as? String ?? "").uppercased() != "COMPLETED" {
                "PENDING"
            } else {
                (entry["conclusion"] as? String ?? "").uppercased()
            }
        } else {
            (entry["state"] as? String ?? "").uppercased()
        }

        switch raw {
        case "SUCCESS",
             "NEUTRAL",
             "SKIPPED":
            return .passing
        case "FAILURE",
             "ERROR",
             "CANCELLED",
             "TIMED_OUT",
             "ACTION_REQUIRED",
             "STARTUP_FAILURE":
            return .failing
        default:
            return .pending
        }
    }
}
