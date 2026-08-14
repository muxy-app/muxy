import Foundation

struct ProfilerSessionRecord: Codable, Equatable, Sendable {
    let schemaVersion: Int
    let recordType: String
    let timestamp: Date
    let appVersion: String
    let appBuild: String
    let macOSVersion: String
    let architecture: String
    let samplingIntervalSeconds: Double

    init(
        timestamp: Date,
        appVersion: String,
        appBuild: String,
        macOSVersion: String,
        architecture: String,
        samplingIntervalSeconds: Double
    ) {
        schemaVersion = 1
        recordType = "session"
        self.timestamp = timestamp
        self.appVersion = appVersion
        self.appBuild = appBuild
        self.macOSVersion = macOSVersion
        self.architecture = architecture
        self.samplingIntervalSeconds = samplingIntervalSeconds
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case recordType = "record_type"
        case timestamp
        case appVersion = "app_version"
        case appBuild = "app_build"
        case macOSVersion = "macos_version"
        case architecture
        case samplingIntervalSeconds = "sampling_interval_seconds"
    }
}

struct ProfilerSampleRecord: Codable, Equatable, Sendable {
    let schemaVersion: Int
    let recordType: String
    let timestamp: Date
    let profilerUptimeSeconds: Double
    let cpuPercent: Double
    let physicalFootprintBytes: UInt64

    init(
        timestamp: Date,
        profilerUptimeSeconds: Double,
        cpuPercent: Double,
        physicalFootprintBytes: UInt64
    ) {
        schemaVersion = 1
        recordType = "sample"
        self.timestamp = timestamp
        self.profilerUptimeSeconds = profilerUptimeSeconds
        self.cpuPercent = cpuPercent
        self.physicalFootprintBytes = physicalFootprintBytes
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case recordType = "record_type"
        case timestamp
        case profilerUptimeSeconds = "profiler_uptime_seconds"
        case cpuPercent = "cpu_percent"
        case physicalFootprintBytes = "physical_footprint_bytes"
    }
}

struct ProfilerProcessMeasurement: Equatable, Sendable {
    let cumulativeCPUNanoseconds: UInt64
    let physicalFootprintBytes: UInt64
}

enum ProfilerCPUCalculator {
    static func percent(
        previousCPUNanoseconds: UInt64,
        currentCPUNanoseconds: UInt64,
        previousMonotonicNanoseconds: UInt64,
        currentMonotonicNanoseconds: UInt64
    ) -> Double? {
        guard currentCPUNanoseconds >= previousCPUNanoseconds,
              currentMonotonicNanoseconds > previousMonotonicNanoseconds
        else { return nil }

        let cpuDelta = Double(currentCPUNanoseconds - previousCPUNanoseconds)
        let wallDelta = Double(currentMonotonicNanoseconds - previousMonotonicNanoseconds)
        return cpuDelta / wallDelta * 100
    }
}

struct ProfilerSampleBuilder {
    private let sessionStartNanoseconds: UInt64
    private var previousMeasurement: ProfilerProcessMeasurement
    private var previousMonotonicNanoseconds: UInt64

    init(
        sessionStartNanoseconds: UInt64,
        baselineMeasurement: ProfilerProcessMeasurement,
        baselineMonotonicNanoseconds: UInt64
    ) {
        self.sessionStartNanoseconds = sessionStartNanoseconds
        previousMeasurement = baselineMeasurement
        previousMonotonicNanoseconds = baselineMonotonicNanoseconds
    }

    mutating func makeSample(
        measurement: ProfilerProcessMeasurement,
        timestamp: Date,
        monotonicNanoseconds: UInt64
    ) throws -> ProfilerSampleRecord {
        guard let cpuPercent = ProfilerCPUCalculator.percent(
            previousCPUNanoseconds: previousMeasurement.cumulativeCPUNanoseconds,
            currentCPUNanoseconds: measurement.cumulativeCPUNanoseconds,
            previousMonotonicNanoseconds: previousMonotonicNanoseconds,
            currentMonotonicNanoseconds: monotonicNanoseconds
        ), monotonicNanoseconds >= sessionStartNanoseconds
        else {
            throw ProfilerRecorderError.invalidMeasurementDelta
        }

        previousMeasurement = measurement
        previousMonotonicNanoseconds = monotonicNanoseconds
        return ProfilerSampleRecord(
            timestamp: timestamp,
            profilerUptimeSeconds: Double(monotonicNanoseconds - sessionStartNanoseconds) / 1_000_000_000,
            cpuPercent: cpuPercent,
            physicalFootprintBytes: measurement.physicalFootprintBytes
        )
    }
}

enum ProfilerRecorderError: Error {
    case invalidMeasurementDelta
}

enum ProfilerRecordEncoder {
    static func encode(_ record: some Encodable) throws -> Data {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(record)
    }
}
