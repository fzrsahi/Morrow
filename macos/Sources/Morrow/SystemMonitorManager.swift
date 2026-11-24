import SwiftUI
import AppKit
import IOKit
import IOKit.ps
import Darwin

<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
public enum PowerAdapterKind: String, CaseIterable, Sendable, Equatable {
    case magSafe
    case usbC
    case ac
=======
public enum PowerSourceKind: String, Sendable, Equatable {
    case magSafe
    case usbC
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift

    public var label: String {
        switch self {
        case .magSafe: return "MagSafe"
        case .usbC: return "USB-C"
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
        case .ac: return "AC Power"
=======
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        }
    }
}

<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
public enum PowerBannerState: Equatable, Sendable {
    case charging(kind: PowerAdapterKind, watts: Int?, timeToFullMinutes: Int?)
    case chargingOnHold(kind: PowerAdapterKind, watts: Int?)
    case fullyCharged(kind: PowerAdapterKind)
    case lowBattery

    public var title: String {
        switch self {
        case .charging(let kind, _, _):
            switch kind {
            case .magSafe: return "MagSafe Charging"
            case .usbC: return "USB-C Charging"
            case .ac: return "Charging"
            }
        case .chargingOnHold: return "Charging On Hold"
=======
public enum PowerBannerState: Sendable, Equatable {
    case charging(kind: PowerSourceKind, watts: Int?, timeToFullMinutes: Int?)
    case onHold(kind: PowerSourceKind)
    case fullyCharged(kind: PowerSourceKind)
    case lowBattery(percent: Int)

    public var title: String {
        switch self {
        case .charging: return "Charging"
        case .onHold: return "Charging On Hold"
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        case .fullyCharged: return "Fully Charged"
        case .lowBattery: return "Low Battery"
        }
    }

    public var subtitle: String {
        switch self {
        case .charging(let kind, let watts, let mins):
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
            var parts: [String] = []
            if let watts, watts > 0 {
                parts.append("\(watts)W \(kind.label)")
            } else {
                parts.append("Charger connected")
            }
            if let mins, mins > 0, mins < 600 {
                parts.append("~\(mins)m left")
            }
            return parts.joined(separator: " · ")
        case .chargingOnHold(let kind, let watts):
            if let watts, watts > 0 {
                return "Optimized Charging · \(watts)W \(kind.label)"
            }
            return "Optimized Charging · \(kind.label)"
        case .fullyCharged(let kind):
            return "\(kind.label) Connected"
=======
            var parts = [kind.label]
            if let watts, watts > 0 { parts.append("\(watts)W") }
            if let mins, mins > 0 {
                let h = mins / 60
                let m = mins % 60
                parts.append(h > 0 ? "\(h)h \(m)m left" : "\(m)m left")
            }
            return parts.joined(separator: " · ")
        case .onHold(let kind):
            return "\(kind.label) · Optimized charging"
        case .fullyCharged(let kind):
            return "\(kind.label) · 100% charged"
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        case .lowBattery:
            return "Connect power adapter"
        }
    }
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
=======

    public var accentColor: Color {
        switch self {
        case .charging, .fullyCharged:
            return Color(red: 0.19, green: 0.82, blue: 0.35)
        case .onHold:
            return Color(red: 1.0, green: 0.72, blue: 0.12)
        case .lowBattery:
            return Color(red: 1.0, green: 0.27, blue: 0.23)
        }
    }

    public var modernIconName: String {
        switch self {
        case .charging: return "bolt.fill"
        case .onHold: return "pause.fill"
        case .fullyCharged: return "checkmark"
        case .lowBattery: return "battery.25"
        }
    }

    public var pixelGlyph: PixelIconGlyph {
        switch self {
        case .charging: return .bolt
        case .onHold, .lowBattery: return .power
        case .fullyCharged: return .checkmark
        }
    }
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
}

public struct SystemMetrics: Sendable, Equatable {
    public var cpuUsagePercent: Double = 0.0
    public var memoryUsedGB: Double = 0.0
    public var memoryTotalGB: Double = 0.0
    public var memoryPercent: Double = 0.0
    public var downloadBytesPerSec: Double = 0.0
    public var uploadBytesPerSec: Double = 0.0
    public var batteryLevel: Int = 100
    public var isCharging: Bool = false
    public var isACPowered: Bool = true
    public var powerBannerState: PowerBannerState = .charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil)
}

@MainActor
public final class SystemMonitorManager: ObservableObject {
    public static let shared = SystemMonitorManager()

    @Published public private(set) var metrics = SystemMetrics()
    @Published public var showChargingBanner: Bool = false
    @Published public private(set) var activeBannerState: PowerBannerState = .charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil)
    @Published public private(set) var activeBannerBatteryLevel: Int = 100
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
    private var chargingDismissWorkItem: DispatchWorkItem?
=======
    private let bannerTimer = BannerHoverHoldTimer(defaultDuration: 3.0)
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift

    public func dismissChargingBanner() {
        bannerTimer.cancel()
        withAnimation(.spring(response: 0.35, dampingFraction: 0.8)) {
            self.showChargingBanner = false
        }
    }

<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
    public func triggerChargingBanner(previewState: PowerBannerState? = nil, previewBatteryLevel: Int? = nil) {
        PrayerTimeManager.shared.dismissMiniPeekBanner()
        if let previewState {
            activeBannerState = previewState
        } else {
            activeBannerState = metrics.powerBannerState
        }
        if let previewBatteryLevel {
            activeBannerBatteryLevel = previewBatteryLevel
        } else {
            activeBannerBatteryLevel = metrics.batteryLevel
        }
        showChargingBanner = true
=======
    public func setChargingBannerHovered(_ hovering: Bool) {
        bannerTimer.setHovered(hovering, isVisible: showChargingBanner) { [weak self] in
            self?.dismissChargingBanner()
        }
    }

    public func triggerChargingBanner(previewState: PowerBannerState? = nil, previewBatteryLevel: Int? = nil) {
        PrayerTimeManager.shared.dismissMiniPeekBanner()
        activeBannerState = previewState ?? metrics.powerBannerState
        activeBannerBatteryLevel = previewBatteryLevel ?? metrics.batteryLevel
        withAnimation(.spring(response: 0.38, dampingFraction: 0.84)) {
            showChargingBanner = true
        }
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)
        bannerTimer.start { [weak self] in
            self?.dismissChargingBanner()
        }
    }

    private var monitorTimer: Timer?
    /// How many gauge views are on screen. Sampling runs only while one is.
    private var samplingViews = 0
    private var previousCpuLoad: host_cpu_load_info?
    private var previousNetworkBytes: (inBytes: UInt64, outBytes: UInt64, time: Date)?
    private var lastChargingState: Bool?
    private var lastACState: Bool?
    private var lastLowBatteryAlerted: Bool = false
    /// IOKit's power-source change source: the charger is an event, not a poll.
    private var powerSourceRunLoopSource: CFRunLoopSource?
    private var isDisplayAsleep = false

    private init() {
        setupPowerSourceNotifications()
        setupSleepObservers()
        // One reading at launch gives the charging banner and the first gauge a
        // fresh value without leaving a timer running.
        refreshMetrics()
    }

    deinit {
        if let source = powerSourceRunLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .defaultMode)
        }
    }

    // Suspend sampling when display is asleep to save battery
    private func setupSleepObservers() {
        let wsCenter = NSWorkspace.shared.notificationCenter
        wsCenter.addObserver(
            forName: NSWorkspace.screensDidSleepNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.isDisplayAsleep = true
                self?.stopMonitoring()
            }
        }
        wsCenter.addObserver(
            forName: NSWorkspace.screensDidWakeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.isDisplayAsleep = false
                self?.startMonitoring()
            }
        }
    }

    /// The charger coming and going is delivered by IOKit; polling for it would
    /// mean waking the app every couple of seconds for the whole day.
    private func setupPowerSourceNotifications() {
        let context = Unmanaged.passUnretained(self).toOpaque()
        guard let source = IOPSNotificationCreateRunLoopSource({ context in
            guard let context else { return }
            let manager = Unmanaged<SystemMonitorManager>.fromOpaque(context).takeUnretainedValue()
            // The source is on the main run loop, so this already is the main thread.
            MainActor.assumeIsolated { manager.powerSourceChanged() }
        }, context)?.takeRetainedValue() else { return }
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .defaultMode)
        powerSourceRunLoopSource = source
    }

    /// A power-source event: the charger was plugged in or pulled. Only the
    /// power fields are re-read; the gauges are not running a timer here.
    private func powerSourceChanged() {
        let (battery, isCharging, isAC, bannerState) = readPowerStatus()
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
        let becameCharging = (lastChargingState == false && isCharging)
        let becameAC = (lastACState == false && isAC)
        let becameLowBattery = (!isAC && battery <= 20 && !lastLowBatteryAlerted)
        if isAC || battery > 25 {
            lastLowBatteryAlerted = false
        } else if becameLowBattery {
            lastLowBatteryAlerted = true
        }

        lastChargingState = isCharging
        lastACState = isAC

=======
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        if metrics.batteryLevel != battery || metrics.isCharging != isCharging || metrics.isACPowered != isAC || metrics.powerBannerState != bannerState {
            metrics.batteryLevel = battery
            metrics.isCharging = isCharging
            metrics.isACPowered = isAC
            metrics.powerBannerState = bannerState
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
        }

        if becameCharging || becameAC || becameLowBattery {
            triggerChargingBanner()
=======
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        }
        if lastChargingState == false, isCharging {
            triggerChargingBanner(previewState: bannerState, previewBatteryLevel: battery)
        }
        lastChargingState = isCharging
    }

    /// A gauge appeared: start sampling, and show a fresh reading at once.
    public func beginSampling() {
        samplingViews += 1
        guard samplingViews == 1 else { return }
        startMonitoring()
    }

    /// A gauge went away: stop sampling when the last one is gone.
    public func endSampling() {
        samplingViews = max(0, samplingViews - 1)
        guard samplingViews == 0 else { return }
        stopMonitoring()
    }

    public func startMonitoring() {
        guard monitorTimer == nil, !isDisplayAsleep, samplingViews > 0 else { return }
        // The first sample is immediate: a gauge that just appeared must not
        // show a stale number for two seconds.
        refreshMetrics()
        let timer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.refreshMetrics()
            }
        }
        // A sample may land a little late; nothing depends on the exact instant.
        timer.tolerance = 0.2
        monitorTimer = timer
    }

    public func stopMonitoring() {
        monitorTimer?.invalidate()
        monitorTimer = nil
    }

    /// Whether a sampling timer is running (tests and diagnostics).
    var isSampling: Bool { monitorTimer != nil }

    public func refreshMetrics() {
        let cpu = readCpuUsage()
        let (memUsed, memTotal, memPct) = readMemoryUsage()
        let (downSpeed, upSpeed) = readNetworkSpeeds()
        let (battery, isCharging, isAC, bannerState) = readPowerStatus()

<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
        let becameCharging = (lastChargingState == false && isCharging)
        let becameAC = (lastACState == false && isAC)
        let becameLowBattery = (!isAC && battery <= 20 && !lastLowBatteryAlerted)
        if isAC || battery > 25 {
            lastLowBatteryAlerted = false
        } else if becameLowBattery {
            lastLowBatteryAlerted = true
=======
        // Check for charger connection event (MagSafe / USB-C animation trigger)
        if let lastState = lastChargingState {
            if !lastState && isCharging {
                triggerChargingBanner(previewState: bannerState, previewBatteryLevel: battery)
            }
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        }
        lastChargingState = isCharging
        lastACState = isAC

        let newMetrics = SystemMetrics(
            cpuUsagePercent: cpu,
            memoryUsedGB: memUsed,
            memoryTotalGB: memTotal,
            memoryPercent: memPct,
            downloadBytesPerSec: downSpeed,
            uploadBytesPerSec: upSpeed,
            batteryLevel: battery,
            isCharging: isCharging,
            isACPowered: isAC,
            powerBannerState: bannerState
        )
        if metrics != newMetrics { metrics = newMetrics }

        if becameCharging || becameAC || becameLowBattery {
            triggerChargingBanner()
        }
    }

    // MARK: - CPU Load via host_statistics

    private func readCpuUsage() -> Double {
        var count = mach_msg_type_number_t(MemoryLayout<host_cpu_load_info_data_t>.size / MemoryLayout<integer_t>.size)
        var cpuLoad = host_cpu_load_info()
        let result = withUnsafeMutablePointer(to: &cpuLoad) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                host_statistics(mach_host_self(), HOST_CPU_LOAD_INFO, $0, &count)
            }
        }
        guard result == KERN_SUCCESS else { return 0.0 }

        defer { previousCpuLoad = cpuLoad }
        guard let prev = previousCpuLoad else { return 0.0 }

        let user = Double(cpuLoad.cpu_ticks.0 - prev.cpu_ticks.0)
        let system = Double(cpuLoad.cpu_ticks.1 - prev.cpu_ticks.1)
        let idle = Double(cpuLoad.cpu_ticks.2 - prev.cpu_ticks.2)
        let nice = Double(cpuLoad.cpu_ticks.3 - prev.cpu_ticks.3)
        let total = user + system + idle + nice
        guard total > 0 else { return 0.0 }
        return ((user + system + nice) / total) * 100.0
    }

    // MARK: - Memory Usage via host_statistics64

    private func readMemoryUsage() -> (usedGB: Double, totalGB: Double, percent: Double) {
        var count = mach_msg_type_number_t(MemoryLayout<vm_statistics64_data_t>.size / MemoryLayout<integer_t>.size)
        var vmStat = vm_statistics64()
        let result = withUnsafeMutablePointer(to: &vmStat) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                host_statistics64(mach_host_self(), HOST_VM_INFO64, $0, &count)
            }
        }
        let totalBytes = ProcessInfo.processInfo.physicalMemory
        let totalGB = Double(totalBytes) / (1024 * 1024 * 1024)

        guard result == KERN_SUCCESS else { return (0, totalGB, 0) }

        let pageSize = Double(vm_kernel_page_size)
        let active = Double(vmStat.active_count) * pageSize
        let wired = Double(vmStat.wire_count) * pageSize
        let compressed = Double(vmStat.compressor_page_count) * pageSize
        let usedBytes = active + wired + compressed
        let usedGB = usedBytes / (1024 * 1024 * 1024)
        let percent = min(100.0, max(0.0, (usedGB / totalGB) * 100.0))

        return (usedGB, totalGB, percent)
    }

    // MARK: - Network Transfer Speeds via getifaddrs

    private func readNetworkSpeeds() -> (downSpeed: Double, upSpeed: Double) {
        var ifap: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifap) == 0, let first = ifap else { return (0, 0) }
        defer { freeifaddrs(ifap) }

        var totalIn: UInt64 = 0
        var totalOut: UInt64 = 0

        var ptr: UnsafeMutablePointer<ifaddrs>? = first
        while let current = ptr {
            let name = String(cString: current.pointee.ifa_name)
            if !name.hasPrefix("lo") && current.pointee.ifa_addr?.pointee.sa_family == UInt8(AF_LINK) {
                if let data = current.pointee.ifa_data {
                    let networkData = data.assumingMemoryBound(to: if_data.self)
                    totalIn += UInt64(networkData.pointee.ifi_ibytes)
                    totalOut += UInt64(networkData.pointee.ifi_obytes)
                }
            }
            ptr = current.pointee.ifa_next
        }

        let now = Date()
        defer { previousNetworkBytes = (totalIn, totalOut, now) }
        guard let prev = previousNetworkBytes else { return (0, 0) }

        let elapsed = max(0.5, now.timeIntervalSince(prev.time))
        let downSpeed = Double(totalIn >= prev.inBytes ? totalIn - prev.inBytes : 0) / elapsed
        let upSpeed = Double(totalOut >= prev.outBytes ? totalOut - prev.outBytes : 0) / elapsed

        return (downSpeed, upSpeed)
    }

    // MARK: - Power & Battery Status via IOKit

    private func readPowerStatus() -> (batteryLevel: Int, isCharging: Bool, isAC: Bool, bannerState: PowerBannerState) {
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
        let (adapterKind, watts) = readExternalPowerAdapterDetails()

=======
        let (adapterKind, adapterWatts) = readExternalAdapterDetails()
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
        guard let snapshot = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
              let sources = IOPSCopyPowerSourcesList(snapshot)?.takeRetainedValue() as? [CFTypeRef],
              !sources.isEmpty else {
            return (100, false, true, .fullyCharged(kind: adapterKind))
        }

        for source in sources {
            guard let desc = IOPSGetPowerSourceDescription(snapshot, source)?.takeUnretainedValue() as? [String: Any] else { continue }
            let curCapacity = desc[kIOPSCurrentCapacityKey as String] as? Int ?? 100
            let isCharging = desc[kIOPSIsChargingKey as String] as? Bool ?? false
            let isCharged = desc[kIOPSIsChargedKey as String] as? Bool ?? false
<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
            let powerSourceState = desc[kIOPSPowerSourceStateKey as String] as? String
            let isAC = (powerSourceState == (kIOPSACPowerValue as String))
            let timeToFull = desc[kIOPSTimeToFullChargeKey as String] as? Int

            let bannerState: PowerBannerState
            if isAC {
                if isCharging {
                    bannerState = .charging(kind: adapterKind, watts: watts, timeToFullMinutes: timeToFull)
                } else if isCharged || curCapacity >= 100 {
                    bannerState = .fullyCharged(kind: adapterKind)
                } else {
                    bannerState = .chargingOnHold(kind: adapterKind, watts: watts)
                }
            } else if curCapacity <= 20 {
                bannerState = .lowBattery
            } else {
                bannerState = .charging(kind: adapterKind, watts: watts, timeToFullMinutes: nil)
            }

=======
            let rawTimeToFull = desc[kIOPSTimeToFullChargeKey as String] as? Int
            let timeToFull = (rawTimeToFull ?? -1) > 0 ? rawTimeToFull : nil
            let powerSourceState = desc[kIOPSPowerSourceStateKey as String] as? String
            let isAC = (powerSourceState == (kIOPSACPowerValue as String))

            let bannerState: PowerBannerState
            if !isCharging && isAC && (isCharged || curCapacity >= 99) {
                bannerState = .fullyCharged(kind: adapterKind)
            } else if !isCharging && isAC {
                bannerState = .onHold(kind: adapterKind)
            } else if !isCharging && !isAC && curCapacity <= 20 {
                bannerState = .lowBattery(percent: curCapacity)
            } else {
                bannerState = .charging(kind: adapterKind, watts: adapterWatts, timeToFullMinutes: timeToFull)
            }
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
            return (curCapacity, isCharging, isAC, bannerState)
        }

        return (100, false, true, .fullyCharged(kind: adapterKind))
    }

<<<<<<< HEAD:Sources/Morrow/SystemMonitorManager.swift
    private func readExternalPowerAdapterDetails() -> (kind: PowerAdapterKind, watts: Int?) {
        guard let details = IOPSCopyExternalPowerAdapterDetails()?.takeRetainedValue() as? [String: Any] else {
            return (.ac, nil)
        }
        let watts = details[kIOPSPowerAdapterWattsKey as String] as? Int
        let name = (details["Name"] as? String ?? details["Description"] as? String ?? "").lowercased()
        let familyCode = details[kIOPSPowerAdapterFamilyKey as String] as? Int

        if name.contains("magsafe") {
            return (.magSafe, watts)
        }
        // Apple USB-C PD family code is 0xe000400a (-536854518 signed)
        if name.contains("usb-c") || name.contains("usb c") || name.contains("pd") || familyCode == Int(bitPattern: 0xe000400a) {
            return (.usbC, watts)
        }
        return (.ac, watts)
=======
    private func isMagSafePortConnected() -> Bool {
        var iterator: io_iterator_t = 0
        guard let matching = IOServiceMatching("IOPort"),
              IOServiceGetMatchingServices(kIOMainPortDefault, matching, &iterator) == KERN_SUCCESS else {
            return false
        }
        defer { IOObjectRelease(iterator) }

        var service = IOIteratorNext(iterator)
        while service != 0 {
            defer {
                IOObjectRelease(service)
                service = IOIteratorNext(iterator)
            }
            guard let desc = IORegistryEntryCreateCFProperty(
                service,
                "PortTypeDescription" as CFString,
                kCFAllocatorDefault,
                0
            )?.takeRetainedValue() as? String,
                  let active = IORegistryEntryCreateCFProperty(
                service,
                "ConnectionActive" as CFString,
                kCFAllocatorDefault,
                0
            )?.takeRetainedValue() as? Bool else {
                continue
            }
            if active && desc.localizedCaseInsensitiveContains("magsafe") {
                return true
            }
        }
        return false
    }

    private func readExternalAdapterDetails() -> (kind: PowerSourceKind, watts: Int?) {
        let magSafeConnected = isMagSafePortConnected()
        guard let details = IOPSCopyExternalPowerAdapterDetails()?.takeRetainedValue() as? [String: Any] else {
            return (magSafeConnected ? .magSafe : .usbC, nil)
        }
        let watts = details[kIOPSPowerAdapterWattsKey as String] as? Int
        let name = (details["Name"] as? String ?? details[kIOPSPowerAdapterSourceKey as String] as? String ?? "").lowercased()
        let desc = (details["Description"] as? String ?? "").lowercased()
        let combined = "\(name) \(desc)"

        let kind: PowerSourceKind
        if magSafeConnected || combined.contains("magsafe") {
            kind = .magSafe
        } else if combined.contains("usb") || combined.contains("pd") || combined.contains("type-c") {
            kind = .usbC
        } else {
            // Apple Silicon adapters with family code 0xE000400A / 0xE0004007 are USB-PD unless named MagSafe
            let family = details[kIOPSPowerAdapterFamilyKey as String] as? Int ?? 0
            kind = (family == 0xE000400A || family == 0xE0004007) ? .usbC : .magSafe
        }
        return (kind, watts)
>>>>>>> origin/main:macos/Sources/Morrow/SystemMonitorManager.swift
    }

    // Formatters
    public static func formatSpeed(_ bytesPerSec: Double) -> String {
        if bytesPerSec >= 1024 * 1024 {
            return String(format: "%.1fM", bytesPerSec / (1024 * 1024))
        } else if bytesPerSec >= 1024 {
            return String(format: "%.0fK", bytesPerSec / 1024)
        } else {
            return "\(Int(bytesPerSec))B"
        }
    }
}
