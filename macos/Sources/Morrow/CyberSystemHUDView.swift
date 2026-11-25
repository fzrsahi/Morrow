import SwiftUI
import AppKit
import MorrowCore

// MARK: - MagSafe / USB-C Charging Dynamic Island Banner

/// Footprint for the compact action button on a notification pill. The two
/// themes keep their own materials, but the tap target stays identical so the
/// control reads the same whether Modern or Pixel is active at runtime.
enum NotificationPillAction {
    static let hitTarget: CGFloat = 24
    static let glyphSize: CGFloat = 9

    static func cornerRadius(isPixel: Bool) -> CGFloat { isPixel ? 4 : 12 }
    static func fill(isPixel: Bool) -> Color {
        isPixel ? Pixel16Palette.surface : Color.white.opacity(0.12)
    }
    static func borderOpacity(isPixel: Bool) -> Double { isPixel ? 0.28 : 0.16 }
}

public struct MagSafeChargingBanner: View {
    let batteryLevel: Int
    var displayMode: IslandDisplayMode = .notch
    var onDismiss: (() -> Void)? = nil
    @ObservedObject private var sys = SystemMonitorManager.shared
    @AppStorage(SettingsKey.islandUITheme) private var islandUITheme = SettingsDefaults.islandUITheme
    @State private var pulseRing = false
    @State private var animatedProgress: Double = 0
    @State private var rippleScale: CGFloat = 0.85
    @State private var rippleOpacity: Double = 0.0
    @State private var displayedPercent: Int = 0
    @State private var countTimer: Timer? = nil

    private var isPixel: Bool {
        islandUITheme == IslandUITheme.pixel.rawValue
    }

    private var effectiveBattery: Int {
        sys.showChargingBanner ? sys.activeBannerBatteryLevel : batteryLevel
    }

    private var effectiveState: PowerBannerState {
        sys.activeBannerState
    }

    public init(batteryLevel: Int, displayMode: IslandDisplayMode = .notch, onDismiss: (() -> Void)? = nil) {
        self.batteryLevel = batteryLevel
        self.displayMode = displayMode
        self.onDismiss = onDismiss
    }

    static func semanticColor(for state: PowerBannerState) -> Color {
        state.accentColor
    }

    var stateColor: Color {
        Self.semanticColor(for: effectiveState)
    }

    private var modernIconName: String {
        effectiveState.modernIconName
    }

    private var pixelGlyph: PixelIconGlyph {
        effectiveState.pixelGlyph
    }

    /// The panel passes a dismiss closure but the pill had no control to call
    /// it. Keep the footprint shared and let each theme supply its own chrome.
    @ViewBuilder
    private var dismissAction: some View {
        if let onDismiss {
            Button {
                NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)
                onDismiss()
            } label: {
                ZStack {
                    RoundedRectangle(
                        cornerRadius: NotificationPillAction.cornerRadius(isPixel: isPixel),
                        style: .continuous
                    )
                    .fill(NotificationPillAction.fill(isPixel: isPixel))
                    .overlay {
                        RoundedRectangle(
                            cornerRadius: NotificationPillAction.cornerRadius(isPixel: isPixel),
                            style: .continuous
                        )
                        .stroke(
                            Color.white.opacity(NotificationPillAction.borderOpacity(isPixel: isPixel)),
                            lineWidth: 1
                        )
                    }

                    Image(systemName: "xmark")
                        .font(.system(size: NotificationPillAction.glyphSize, weight: .bold))
                        .foregroundStyle(Color.white.opacity(isPixel ? 0.85 : 0.72))
                }
                .frame(
                    width: NotificationPillAction.hitTarget,
                    height: NotificationPillAction.hitTarget
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Dismiss charging notification")
            .accessibilityLabel("Dismiss charging notification")
        }
    }

    public var body: some View {
        Group {
            if isPixel {
                pixelChargingBanner
            } else {
                modernChargingBanner
            }
        }
        .contentShape(Rectangle())
        .onHover { hovering in
            SystemMonitorManager.shared.setChargingBannerHovered(hovering)
        }
        .onAppear {
            startEntryAnimations()
        }
        .onDisappear {
            countTimer?.invalidate()
            countTimer = nil
        }
        .onChange(of: effectiveBattery) { _, _ in
            startEntryAnimations()
        }
    }

    private func startEntryAnimations() {
        let targetPct = max(0.06, min(1.0, Double(effectiveBattery) / 100.0))
        animatedProgress = 0
        rippleScale = 0.82
        rippleOpacity = 0.55

        withAnimation(.spring(response: 0.72, dampingFraction: 0.82)) {
            animatedProgress = targetPct
        }
        withAnimation(.easeOut(duration: 0.75)) {
            rippleScale = 1.42
            rippleOpacity = 0.0
        }
        withAnimation(.easeInOut(duration: 0.85).repeatForever(autoreverses: true)) {
            pulseRing = true
        }

        // Smooth rolling % counter synced with the ring sweep
        countTimer?.invalidate()
        let end = max(0, min(100, effectiveBattery))
        let start = max(0, end - 24)
        displayedPercent = start
        guard end > start else {
            displayedPercent = end
            return
        }
        let steps = min(18, end - start)
        let interval = 0.48 / Double(steps)
        var stepIdx = 0
        countTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { timer in
            Task { @MainActor in
                stepIdx += 1
                if stepIdx >= steps {
                    displayedPercent = end
                    timer.invalidate()
                } else {
                    let frac = Double(stepIdx) / Double(steps)
                    displayedPercent = start + Int((Double(end - start) * frac).rounded())
                }
            }
        }
    }

    // MARK: - 1. Pixel 32-Bit Power Banner

    private var pixelChargingBanner: some View {
        HStack(spacing: 12) {
            ZStack {
                Canvas { ctx, _ in
                    let px: CGFloat = 2.0
                    let wallColor = Color.white.opacity(0.34)

                    // 20x17 integer-grid 16-bit pixel battery casing
                    ctx.fill(Path(CGRect(x: 1 * px, y: 3 * px, width: 16 * px, height: 1 * px)), with: .color(wallColor))
                    ctx.fill(Path(CGRect(x: 1 * px, y: 13 * px, width: 16 * px, height: 1 * px)), with: .color(wallColor))
                    ctx.fill(Path(CGRect(x: 1 * px, y: 3 * px, width: 1 * px, height: 11 * px)), with: .color(wallColor))
                    ctx.fill(Path(CGRect(x: 16 * px, y: 3 * px, width: 1 * px, height: 11 * px)), with: .color(wallColor))
                    // Positive terminal nub on right
                    ctx.fill(Path(CGRect(x: 17 * px, y: 6 * px, width: 2 * px, height: 5 * px)), with: .color(wallColor))

                    // 4 crisp internal pixel energy blocks
                    let blocks = 4
                    let ratio = Double(effectiveBattery) / 100.0
                    let litCount = max(1, Int((ratio * Double(blocks)).rounded()))
                    for i in 0..<blocks {
                        let blockColor = i < litCount ? stateColor : Color.white.opacity(0.10)
                        let bx = (3.0 + Double(i) * 3.2) * px
                        ctx.fill(
                            Path(CGRect(x: bx, y: 5 * px, width: 2.4 * px, height: 7 * px)),
                            with: .color(blockColor)
                        )
                    }

                    // 7x7 crisp integer-grid status glyph centered inside the battery (matches Opsi B preview)
                    let glyphMatrix: [[Int]] = {
                        switch effectiveState {
                        case .charging:
                            return [
                                [0,0,0,1,1,0,0],
                                [0,0,1,1,0,0,0],
                                [0,1,1,0,0,0,0],
                                [1,1,1,1,1,1,0],
                                [0,0,0,1,1,0,0],
                                [0,0,1,1,0,0,0],
                                [0,1,1,0,0,0,0]
                            ]
                        case .onHold:
                            return [
                                [0,1,1,0,1,1,0],
                                [0,1,1,0,1,1,0],
                                [0,1,1,0,1,1,0],
                                [0,1,1,0,1,1,0],
                                [0,1,1,0,1,1,0],
                                [0,1,1,0,1,1,0],
                                [0,1,1,0,1,1,0]
                            ]
                        case .fullyCharged:
                            return [
                                [0,0,0,0,0,0,1],
                                [0,0,0,0,0,1,1],
                                [0,0,0,0,1,1,0],
                                [1,0,0,1,1,0,0],
                                [1,1,1,1,0,0,0],
                                [0,1,1,0,0,0,0],
                                [0,0,0,0,0,0,0]
                            ]
                        case .lowBattery:
                            return [
                                [0,0,1,1,1,0,0],
                                [0,0,1,1,1,0,0],
                                [0,0,1,1,1,0,0],
                                [0,0,1,1,1,0,0],
                                [0,0,0,0,0,0,0],
                                [0,0,1,1,1,0,0],
                                [0,0,1,1,1,0,0]
                            ]
                        }
                    }()

                    for (r, row) in glyphMatrix.enumerated() {
                        for (c, bit) in row.enumerated() where bit == 1 {
                            ctx.fill(
                                Path(CGRect(
                                    x: (5.5 + Double(c)) * px,
                                    y: (5.0 + Double(r)) * px,
                                    width: px,
                                    height: px
                                )),
                                with: .color(.white)
                            )
                        }
                    }
                }
                .frame(width: 40, height: 40)
            }
            .layoutPriority(2)

            VStack(alignment: .leading, spacing: 3) {
                PixelText(text: effectiveState.title.uppercased(), color: .white, pixelSize: 1.45)
                Text(effectiveState.subtitle)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(Color.white.opacity(0.58))
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
            .layoutPriority(0)

            Spacer(minLength: 4)

            PixelText(text: "\(displayedPercent)%", color: .white, pixelSize: 2.2)
                .fixedSize()
                .layoutPriority(2)

            dismissAction
                .padding(.leading, 2)
        }
        .padding(.horizontal, displayMode == .dynamicIsland ? 18 : 16)
        .padding(.vertical, displayMode == .dynamicIsland ? 8 : 10)
    }

    // MARK: - 2. Modern Clean Dynamic Island Banner

    private var modernChargingBanner: some View {
        let pct = CGFloat(animatedProgress)

        return HStack(spacing: 12) {
            ZStack {
                Circle()
                    .stroke(stateColor.opacity(rippleOpacity), lineWidth: 2.0)
                    .frame(width: 40, height: 40)
                    .scaleEffect(rippleScale)

                Circle()
                    .stroke(stateColor.opacity(0.22), lineWidth: 4.5)
                    .frame(width: 40, height: 40)

                Circle()
                    .trim(from: 0.0, to: pct)
                    .stroke(stateColor, style: StrokeStyle(lineWidth: 4.5, lineCap: .round))
                    .frame(width: 40, height: 40)
                    .rotationEffect(.degrees(-90))
                    .shadow(color: stateColor.opacity(0.45), radius: 4)

                Image(systemName: modernIconName)
                    .font(.system(size: 13.5, weight: .bold))
                    .foregroundStyle(.white)
                    .scaleEffect(pulseRing ? 1.07 : 0.95)
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(effectiveState.title)
                    .font(.system(size: 15.5, weight: .bold, design: .rounded))
                    .foregroundStyle(.white)
                    .lineLimit(1)

                Text(effectiveState.subtitle)
                    .font(.system(size: 12, weight: .medium, design: .rounded))
                    .foregroundStyle(Color.white.opacity(0.56))
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            .frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
            .layoutPriority(0)

            Spacer(minLength: 4)

            Text("\(displayedPercent)%")
                .font(.system(size: 23, weight: .regular, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(.white)
                .contentTransition(.numericText())
                .fixedSize()
                .layoutPriority(2)

            dismissAction
                .padding(.leading, 2)
        }
        .padding(.horizontal, displayMode == .dynamicIsland ? 18 : 16)
        .padding(.vertical, displayMode == .dynamicIsland ? 8 : 10)
    }
}

// MARK: - Compact Cyber System Telemetry Gauge Bar

public struct CyberSystemGaugeBar: View {
    @ObservedObject private var sys = SystemMonitorManager.shared

    public init() {}

    static func batteryColor(for metrics: SystemMetrics) -> Color {
        if case .onHold = metrics.powerBannerState {
            return PowerBannerState.onHold(kind: .magSafe).accentColor
        }
        if case .fullyCharged = metrics.powerBannerState {
            return PowerBannerState.fullyCharged(kind: .magSafe).accentColor
        }
        if !metrics.isCharging && metrics.batteryLevel <= 20 {
            return PowerBannerState.lowBattery(percent: metrics.batteryLevel).accentColor
        }
        if metrics.isCharging {
            return PowerBannerState.charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil).accentColor
        }
        return Color.white.opacity(0.8)
    }

    static func batteryIcon(for metrics: SystemMetrics) -> String {
        if metrics.isCharging { return "battery.100.bolt" }
        if case .fullyCharged = metrics.powerBannerState { return "battery.100" }
        if metrics.batteryLevel <= 20 { return "battery.25" }
        if metrics.batteryLevel >= 99 { return "battery.100" }
        return "battery.75"
    }

    /// Sampling runs only while this gauge is on screen: appearing starts it,
    /// disappearing (the Workspace tab closing) stops it.
    private struct SamplingLifecycle: ViewModifier {
        func body(content: Content) -> some View {
            content
                .onAppear { SystemMonitorManager.shared.beginSampling() }
                .onDisappear { SystemMonitorManager.shared.endSampling() }
        }
    }

    public var body: some View {
        let m = sys.metrics
        HStack(spacing: 8) {
            // CPU
            HStack(spacing: 3) {
                Image(systemName: "cpu")
                    .font(.system(size: 7, weight: .bold))
                    .foregroundStyle(m.cpuUsagePercent > 75 ? Color(red: 1.0, green: 0.16, blue: 0.53) : Color(red: 0.0, green: 0.96, blue: 0.83))
                Text(String(format: "%.0f%%", m.cpuUsagePercent))
                    .font(.system(size: 7.5, weight: .bold, design: .monospaced))
                    .foregroundStyle(Color.white)
            }

            // RAM
            HStack(spacing: 3) {
                Image(systemName: "memorychip")
                    .font(.system(size: 7, weight: .bold))
                    .foregroundStyle(Color(red: 0.0, green: 0.90, blue: 0.46))
                Text(String(format: "%.1fG", m.memoryUsedGB))
                    .font(.system(size: 7.5, weight: .bold, design: .monospaced))
                    .foregroundStyle(Color.white.opacity(0.9))
            }

            // Network Speed
            HStack(spacing: 3) {
                Image(systemName: "arrow.down")
                    .font(.system(size: 6.5, weight: .bold))
                    .foregroundStyle(Color.white.opacity(0.6))
                Text(SystemMonitorManager.formatSpeed(m.downloadBytesPerSec))
                    .font(.system(size: 7.5, weight: .bold, design: .monospaced))
                    .foregroundStyle(Color(red: 0.0, green: 0.96, blue: 0.83))
            }

            // Battery
            let batteryColor = Self.batteryColor(for: m)
            let batteryIcon = Self.batteryIcon(for: m)
            HStack(spacing: 2) {
                Image(systemName: batteryIcon)
                    .font(.system(size: 7.5, weight: .bold))
                    .foregroundStyle(batteryColor)
                Text("\(m.batteryLevel)%")
                    .font(.system(size: 7.5, weight: .bold, design: .monospaced))
                    .foregroundStyle(Color.white.opacity(0.85))
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2.5)
        .background(
            RoundedRectangle(cornerRadius: 3.5)
                .fill(Color(white: 0.08))
                .overlay(RoundedRectangle(cornerRadius: 3.5).stroke(Color(white: 0.16), lineWidth: 0.6))
        )
        .modifier(SamplingLifecycle())
    }
}
