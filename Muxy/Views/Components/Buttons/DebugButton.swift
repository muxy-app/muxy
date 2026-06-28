import Darwin
import SwiftUI

struct DebugButton: View {
    @State private var showingPopover = false
    @State private var hovered = false

    var body: some View {
        Button {
            showingPopover.toggle()
        } label: {
            Image(systemName: "ladybug.fill")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.warning : MuxyTheme.warning.opacity(0.75))
                .frame(width: UIMetrics.scaled(22), height: UIMetrics.scaled(22))
                .contentShape(Rectangle())
                .background(hovered ? MuxyTheme.hover : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help("Debug Info")
        .popover(isPresented: $showingPopover, arrowEdge: .bottom) {
            DebugInfoPopover()
        }
    }
}

private struct DebugInfoPopover: View {
    @State private var snapshot = DebugMetrics.current()
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "ladybug.fill")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.warning)
                Text("Debug")
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer(minLength: UIMetrics.spacing6)
                Text("DEV")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                    .foregroundStyle(MuxyTheme.bg)
                    .padding(.horizontal, UIMetrics.scaled(5))
                    .padding(.vertical, UIMetrics.scaled(1))
                    .background(MuxyTheme.warning, in: Capsule())
            }

            VStack(spacing: UIMetrics.spacing3) {
                metricRow("Memory", value: snapshot.memoryString, icon: "memorychip")
                metricRow("CPU", value: snapshot.cpuString, icon: "cpu")
                metricRow("Threads", value: "\(snapshot.threadCount)", icon: "rectangle.split.3x1")
                metricRow("Uptime", value: snapshot.uptimeString, icon: "clock")
            }
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(220))
        .background(MuxyTheme.bg)
        .onReceive(timer) { _ in
            snapshot = DebugMetrics.current()
        }
    }

    private func metricRow(_ label: String, value: String, icon: String) -> some View {
        HStack(spacing: UIMetrics.spacing4) {
            Image(systemName: icon)
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconMD)
            Text(label)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            Spacer(minLength: UIMetrics.spacing4)
            Text(value)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
        }
    }
}

private struct DebugMetrics {
    let memoryBytes: UInt64
    let cpuPercent: Double
    let threadCount: Int
    let uptimeSeconds: TimeInterval

    var memoryString: String {
        ByteCountFormatter.string(fromByteCount: Int64(memoryBytes), countStyle: .memory)
    }

    var cpuString: String {
        String(format: "%.1f%%", cpuPercent)
    }

    var uptimeString: String {
        let total = Int(uptimeSeconds)
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        let seconds = total % 60
        if hours > 0 {
            return String(format: "%dh %02dm", hours, minutes)
        }
        if minutes > 0 {
            return String(format: "%dm %02ds", minutes, seconds)
        }
        return "\(seconds)s"
    }

    static func current() -> DebugMetrics {
        DebugMetrics(
            memoryBytes: residentMemory(),
            cpuPercent: cpuUsage(),
            threadCount: threadCount(),
            uptimeSeconds: Date().timeIntervalSince(MuxyApp.launchDate)
        )
    }

    private static func residentMemory() -> UInt64 {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        return result == KERN_SUCCESS ? info.phys_footprint : 0
    }

    private static func cpuUsage() -> Double {
        var threadList: thread_act_array_t?
        var threadCount = mach_msg_type_number_t(0)
        guard task_threads(mach_task_self_, &threadList, &threadCount) == KERN_SUCCESS,
              let threads = threadList
        else { return 0 }

        defer {
            vm_deallocate(
                mach_task_self_,
                vm_address_t(UInt(bitPattern: threads)),
                vm_size_t(Int(threadCount) * MemoryLayout<thread_t>.stride)
            )
        }

        var totalUsage: Double = 0
        for index in 0 ..< Int(threadCount) {
            var info = thread_basic_info()
            var infoCount = mach_msg_type_number_t(MemoryLayout<thread_basic_info_data_t>.size / MemoryLayout<natural_t>.size)
            let kerr = withUnsafeMutablePointer(to: &info) {
                $0.withMemoryRebound(to: integer_t.self, capacity: Int(infoCount)) {
                    thread_info(threads[index], thread_flavor_t(THREAD_BASIC_INFO), $0, &infoCount)
                }
            }
            guard kerr == KERN_SUCCESS, info.flags & TH_FLAGS_IDLE == 0 else { continue }
            totalUsage += Double(info.cpu_usage) / Double(TH_USAGE_SCALE) * 100
        }
        return totalUsage
    }

    private static func threadCount() -> Int {
        var threadList: thread_act_array_t?
        var count = mach_msg_type_number_t(0)
        guard task_threads(mach_task_self_, &threadList, &count) == KERN_SUCCESS,
              let threads = threadList
        else { return 0 }
        vm_deallocate(
            mach_task_self_,
            vm_address_t(UInt(bitPattern: threads)),
            vm_size_t(Int(count) * MemoryLayout<thread_t>.stride)
        )
        return Int(count)
    }
}
