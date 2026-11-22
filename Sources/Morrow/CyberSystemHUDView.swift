import SwiftUI
import AppKit
import MorrowCore

// MARK: - MagSafe / USB-C Charging Dynamic Island Banner

public struct MagSafeChargingBanner: View {
    let batteryLevel: Int
    var displayMode: IslandDisplayMode = .notch
    var onDismiss: (() -> Void)? = nil
    @ObservedObject private var sys = SystemMonitorManager.shared
    @ObservedObject private var nowPlaying = NowPlayingManager.shared
    @AppStorage(SettingsKey.islandUITheme) private var islandUITheme = SettingsDefaults.islandUITheme
    @AppStorage(SettingsKey.musicHUDProgressColor) private var musicHUDProgressColor = SettingsDefaults.musicHUDProgressColor
    @AppStorage(SettingsKey.syncThemeWithAlbumArt) private var syncThemeWithAlbumArt = SettingsDefaults.syncThemeWithAlbumArt
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

    private var stateColor: Color {
        switch effectiveState {
        case .charging:
            if (syncThemeWithAlbumArt || musicHUDProgressColor == "album") && nowPlaying.isPlaying && nowPlaying.artworkImage != nil {
                return nowPlaying.albumPalette.primary
            }
            return Color(red: 0.19, green: 0.82, blue: 0.35)
        case .fullyCharged:
            return Color(red: 0.19, green: 0.82, blue: 0.35)
        case .lowBattery:
            return Color(red: 1.0, green: 0.62, blue: 0.04)
        }
    }

    private var modernIconName: String {
        switch effectiveState {
        case .charging: return "bolt.fill"
        case .fullyCharged: return "checkmark"
        case .lowBattery: return "battery.25"
        }
    }

    private var pixelGlyph: PixelIconGlyph {
        switch effectiveState {
        case .charging: return .bolt
        case .fullyCharged: return .checkmark
        case .lowBattery: return .power
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
        let pct = animatedProgress

        return HStack(spacing: 12) {
            ZStack {
                Rectangle()
                    .stroke(stateColor.opacity(rippleOpacity), lineWidth: 1.5)
                    .frame(width: 38, height: 38)
                    .scaleEffect(rippleScale)

                Canvas { ctx, size in
                    let cx = size.width / 2
                    let cy = size.height / 2
                    let radius: CGFloat = 16.0
                    let steps = 64
                    for i in 0..<steps {
                        let t = Double(i) / Double(steps)
                        let angle = t * Double.pi * 2.0 - Double.pi / 2.0
                        let px = (cx + cos(angle) * radius).rounded()
                        let py = (cy + sin(angle) * radius).rounded()
                        let rect = CGRect(x: px - 1.2, y: py - 1.2, width: 2.4, height: 2.4)
                        if t <= pct {
                            ctx.fill(Path(rect), with: .color(stateColor))
                        } else {
                            ctx.fill(Path(rect), with: .color(Color.white.opacity(0.14)))
                        }
                    }
                }
                .frame(width: 42, height: 42)

                PixelIcon(glyph: pixelGlyph, color: .white, pixelSize: 1.5)
                    .scaleEffect(pulseRing ? 1.07 : 0.95)
            }

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
        }
        .padding(.horizontal, displayMode == .dynamicIsland ? 20 : 16)
        .padding(.vertical, displayMode == .dynamicIsland ? 12 : 10)
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
        }
        .padding(.horizontal, displayMode == .dynamicIsland ? 20 : 16)
        .padding(.vertical, displayMode == .dynamicIsland ? 12 : 10)
    }
}

// MARK: - Compact Cyber System Telemetry Gauge Bar

public struct CyberSystemGaugeBar: View {
    @ObservedObject private var sys = SystemMonitorManager.shared

    public init() {}

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
            HStack(spacing: 2) {
                Image(systemName: m.isCharging ? "battery.100.bolt" : "battery.75")
                    .font(.system(size: 7.5, weight: .bold))
                    .foregroundStyle(m.isCharging ? Color(red: 0.0, green: 0.90, blue: 0.46) : Color.white.opacity(0.8))
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
