import Foundation
import Testing

@testable import Muxy

@Suite("Profiler records")
struct ProfilerRecordsTests {
    @Test("session and sample JSON contain only anonymous schema fields")
    func anonymousJSONFieldSet() throws {
        let session = makeSession()
        let sample = makeSample()

        let sessionObject = try jsonObject(ProfilerRecordEncoder.encode(session))
        let sampleObject = try jsonObject(ProfilerRecordEncoder.encode(sample))

        #expect(Set(sessionObject.keys) == [
            "schema_version",
            "record_type",
            "timestamp",
            "app_version",
            "app_build",
            "macos_version",
            "architecture",
            "sampling_interval_seconds",
        ])
        #expect(Set(sampleObject.keys) == [
            "schema_version",
            "record_type",
            "timestamp",
            "profiler_uptime_seconds",
            "cpu_percent",
            "physical_footprint_bytes",
        ])
        #expect(sessionObject["record_type"] as? String == "session")
        #expect(sampleObject["record_type"] as? String == "sample")
        #expect(sessionObject["schema_version"] as? Int == 1)
        #expect(sampleObject["schema_version"] as? Int == 1)
    }

    @Test("CPU and uptime use the baseline and monotonic deltas")
    func cpuDeltaBehavior() throws {
        var builder = ProfilerSampleBuilder(
            sessionStartNanoseconds: 99_000_000_000,
            baselineMeasurement: ProfilerProcessMeasurement(
                cumulativeCPUNanoseconds: 10_000_000_000,
                physicalFootprintBytes: 100
            ),
            baselineMonotonicNanoseconds: 100_000_000_000
        )

        let sample = try builder.makeSample(
            measurement: ProfilerProcessMeasurement(
                cumulativeCPUNanoseconds: 10_500_000_000,
                physicalFootprintBytes: 200
            ),
            timestamp: Date(timeIntervalSince1970: 2_000),
            monotonicNanoseconds: 102_000_000_000
        )

        #expect(sample.cpuPercent == 25)
        #expect(sample.profilerUptimeSeconds == 3)
        #expect(sample.physicalFootprintBytes == 200)
        #expect(ProfilerCPUCalculator.percent(
            previousCPUNanoseconds: 20,
            currentCPUNanoseconds: 19,
            previousMonotonicNanoseconds: 10,
            currentMonotonicNanoseconds: 11
        ) == nil)
        #expect(ProfilerCPUCalculator.percent(
            previousCPUNanoseconds: 20,
            currentCPUNanoseconds: 21,
            previousMonotonicNanoseconds: 10,
            currentMonotonicNanoseconds: 10
        ) == nil)
    }

    private func jsonObject(_ data: Data) throws -> [String: Any] {
        try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func makeSession() -> ProfilerSessionRecord {
        ProfilerSessionRecord(
            timestamp: Date(timeIntervalSince1970: 1_000),
            appVersion: "1.2.3",
            appBuild: "456",
            macOSVersion: "15.4.0",
            architecture: "arm64",
            samplingIntervalSeconds: 60
        )
    }

    private func makeSample() -> ProfilerSampleRecord {
        ProfilerSampleRecord(
            timestamp: Date(timeIntervalSince1970: 1_060),
            profilerUptimeSeconds: 60,
            cpuPercent: 12.5,
            physicalFootprintBytes: 123_456
        )
    }
}
