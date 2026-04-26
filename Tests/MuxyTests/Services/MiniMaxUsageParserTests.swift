import Foundation
import Testing

@testable import Muxy

@Suite("MiniMaxUsageParser")
struct MiniMaxUsageParserTests {
    @Test("parses remains payload into session row")
    func parseRemainsPayload() throws {
        let json = """
        {
          "model_remains": [
            {
              "current_interval_total_count": 100,
              "current_interval_usage_count": 40,
              "end_time": 1735779600,
              "current_subscribe_title": "Pro"
            }
          ]
        }
        """

        let rows = try MiniMaxUsageParser.parseMetricRows(from: Data(json.utf8))
        #expect(rows.count == 1)
        #expect(rows[0].label == "Session (Pro)")
        #expect(rows[0].percent == 60)
        #expect(rows[0].detail == "60.0/100")
        #expect(rows[0].resetDate != nil)
    }

    @Test("parses nested remains payload and API status")
    func parseNestedRemainsPayload() throws {
        let json = """
        {
          "base_resp": { "status_code": 0 },
          "data": {
            "result": {
              "model_remains": [
                {
                  "current_interval_total_count": 200,
                  "current_interval_usage_count": 80,
                  "end_time": "2026-04-21T10:00:00Z",
                  "plan_name": "Ultra"
                }
              ]
            }
          }
        }
        """

        let rows = try MiniMaxUsageParser.parseMetricRows(from: Data(json.utf8))
        #expect(rows.count == 1)
        #expect(rows[0].label == "Session (Ultra)")
        #expect(rows[0].percent == 60)
        #expect(rows[0].resetDate != nil)
    }

    @Test("throws API error when base_resp status_code is non-zero")
    func parseThrowsAPIError() throws {
        let json = """
        {
          "base_resp": { "status_code": 1001, "status_msg": "invalid token" }
        }
        """

        do {
            _ = try MiniMaxUsageParser.parseMetricRows(from: Data(json.utf8))
            Issue.record("Expected MiniMaxUsageParserError.apiError")
        } catch let MiniMaxUsageParserError.apiError(message) {
            #expect(message == "invalid token")
        }
    }

    @Test("maps auth-like API status to auth error")
    func parseAuthStatusAsAuthError() throws {
        let json = """
        {
          "base_resp": { "status_code": 1004, "status_msg": "please login again" }
        }
        """

        do {
            _ = try MiniMaxUsageParser.parseMetricRows(from: Data(json.utf8))
            Issue.record("Expected MiniMaxUsageParserError.authError")
        } catch MiniMaxUsageParserError.authError {
            // expected
        }
    }

    @Test("scales CN model-call counts into prompts")
    func parseCNCountsAsPrompts() throws {
        let json = """
        {
          "model_remains": [
            {
              "current_interval_total_count": 1500,
              "current_interval_usage_count": 1200,
              "plan_name": "Plus"
            }
          ]
        }
        """

        let rows = try MiniMaxUsageParser.parseMetricRows(from: Data(json.utf8), region: .cn)
        #expect(rows.count == 1)
        #expect(rows[0].detail == "20.0/100")
        #expect(rows[0].percent == 20)
    }

    @Test("treats remains_time as duration fallback when end_time is absent")
    func parseRemainsTimeAsDuration() throws {
        let json = """
        {
          "model_remains": [
            {
              "current_interval_total_count": 100,
              "current_interval_usage_count": 80,
              "remains_time": 7200
            }
          ]
        }
        """

        let before = Date()
        let rows = try MiniMaxUsageParser.parseMetricRows(from: Data(json.utf8))
        let after = Date()

        #expect(rows.count == 1)
        #expect(rows[0].resetDate != nil)

        if let resetDate = rows[0].resetDate {
            let minExpected = before.addingTimeInterval(7195)
            let maxExpected = after.addingTimeInterval(7205)
            #expect(resetDate >= minExpected)
            #expect(resetDate <= maxExpected)
        }
    }

    @Test("reads minimax token from env")
    func readTokenFromEnv() throws {
        let token = try MiniMaxUsageProvider.readToken(
            env: ["MINIMAX_API_KEY": "mmx-key"],
            homeDirectory: "/mock-home",
            fileExists: { _ in false },
            dataReader: { _ in throw NSError(domain: "test", code: 1) }
        )
        #expect(token == "mmx-key")
    }

    @Test("prefers CN key when present")
    func readTokenPrefersCNKey() throws {
        let token = try MiniMaxUsageProvider.readToken(
            env: [
                "MINIMAX_API_KEY": "global-key",
                "MINIMAX_CN_API_KEY": "cn-key",
            ],
            homeDirectory: "/mock-home",
            fileExists: { _ in false },
            dataReader: { _ in throw NSError(domain: "test", code: 1) }
        )
        #expect(token == "cn-key")
    }
}
