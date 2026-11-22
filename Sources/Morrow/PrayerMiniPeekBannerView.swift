import SwiftUI
import MorrowCore

/// Compact Horizon Sun-Arc Notification Banner for Prayer times.
/// Adapts the single-canvas Sun-Arc & 5-prayer baseline into a below-notch notification banner
/// with Modern Clean and Pixel 32-bit themes, hover-hold persistence, and zero dismiss button clutter.
public struct PrayerMiniPeekBannerView: View {
    @ObservedObject var manager: PrayerTimeManager
    var displayMode: IslandDisplayMode = .notch
    var onOpenFull: () -> Void

    @AppStorage(SettingsKey.islandUITheme) private var islandUITheme = SettingsDefaults.islandUITheme
    @AppStorage(SettingsKey.musicHUDProgressColor) private var musicHUDProgressColor = SettingsDefaults.musicHUDProgressColor
    @AppStorage(SettingsKey.syncThemeWithAlbumArt) private var syncThemeWithAlbumArt = SettingsDefaults.syncThemeWithAlbumArt
    @ObservedObject private var nowPlaying = NowPlayingManager.shared

    private var isPixel: Bool {
        islandUITheme == IslandUITheme.pixel.rawValue
    }

    private var pixelAccent: Color {
        _ = syncThemeWithAlbumArt
        _ = nowPlaying.albumPalette
        return AppColorTheme.readableOnDark(
            AppColorTheme.accent(for: musicHUDProgressColor, isModern: false)
        )
    }

    public init(manager: PrayerTimeManager, displayMode: IslandDisplayMode = .notch, onOpenFull: @escaping () -> Void) {
        self.manager = manager
        self.displayMode = displayMode
        self.onOpenFull = onOpenFull
    }

    private var activePrayerType: PrayerType {
        if let next = manager.upcomingPrayer {
            return next.prayer
        }
        let lower = manager.miniPeekBannerTitle.lowercased()
        for prayer in [PrayerType.subuh, .dzuhur, .ashar, .maghrib, .isya] {
            if lower.contains(prayer.rawValue) {
                return prayer
            }
        }
        return .maghrib
    }

    private var statusSubtitle: String {
        let diff = manager.minutesUntilNextPrayer
        let prefix = diff <= 0 ? "NOW" : "-\(diff)m"
        let sub = manager.miniPeekBannerSubtitle.trimmingCharacters(in: .whitespacesAndNewlines)
        if sub.isEmpty {
            return "\(prefix) · Saatnya Salat \(activePrayerType.displayName)"
        }
        return "\(prefix) · \(sub)"
    }

    private struct PrayerSlot: Identifiable {
        let type: PrayerType
        let shortCode: String
        let date: Date?
        var id: String { type.rawValue }
    }

    private var prayerSlots: [PrayerSlot] {
        let schedule = manager.todaySchedule
        return [
            PrayerSlot(type: .subuh, shortCode: "SUB", date: schedule?.subuh),
            PrayerSlot(type: .dzuhur, shortCode: "DZH", date: schedule?.dzuhur),
            PrayerSlot(type: .ashar, shortCode: "ASH", date: schedule?.ashar),
            PrayerSlot(type: .maghrib, shortCode: "MGH", date: schedule?.maghrib),
            PrayerSlot(type: .isya, shortCode: "ISY", date: schedule?.isya),
        ]
    }

    private var activePrayerTimeString: String {
        if let match = prayerSlots.first(where: { $0.type == activePrayerType })?.date {
            return manager.formattedPrayerTime(match)
        }
        if let next = manager.upcomingPrayer {
            return manager.formattedPrayerTime(next.date)
        }
        return manager.currentTimeString
    }

    private var arcProgress: Double {
        let minH = manager.sunriseHour > 0 ? max(4.0, manager.sunriseHour - 1.2) : 4.5
        let maxH = manager.sunsetHour > minH ? max(18.5, manager.sunsetHour + 1.2) : 19.2
        let clamped = min(max(manager.currentFractionalHour, minH), maxH)
        return (clamped - minH) / max(0.01, maxH - minH)
    }

    private func slotProgress(_ slot: PrayerSlot, index: Int) -> Double {
        if let date = slot.date {
            let comps = Calendar.current.dateComponents([.hour, .minute], from: date)
            let hour = Double(comps.hour ?? 0) + Double(comps.minute ?? 0) / 60.0
            let minH = manager.sunriseHour > 0 ? max(4.0, manager.sunriseHour - 1.2) : 4.5
            let maxH = manager.sunsetHour > minH ? max(18.5, manager.sunsetHour + 1.2) : 19.2
            return min(1.0, max(0.0, (hour - minH) / max(0.01, maxH - minH)))
        }
        return [0.02, 0.44, 0.67, 0.87, 0.98][min(index, 4)]
    }

    public var body: some View {
        Button(action: {
            NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)
            onOpenFull()
        }) {
            Group {
                if isPixel {
                    pixelBanner
                } else {
                    modernBanner
                }
            }
        }
        .buttonStyle(.plain)
        .contentShape(Rectangle())
        .onHover { hovering in
            manager.setMiniPeekBannerHovered(hovering)
        }
    }

    // MARK: - 1. Pixel 32-Bit Compact Sun-Arc Banner

    private var pixelBanner: some View {
        let accent = pixelAccent
        return VStack(spacing: 3) {
            // Top context row
            HStack {
                PixelText(
                    text: "PRAYER · \(manager.cityName.uppercased())",
                    color: Color.white.opacity(0.58),
                    pixelSize: 1.05
                )
                Spacer(minLength: 8)
                PixelText(
                    text: manager.currentTimeString,
                    color: Color.white.opacity(0.76),
                    pixelSize: 1.1
                )
            }

            // Horizon Sun-Arc + Centered Hero
            ZStack(alignment: .bottom) {
                pixelSunArc(accent: accent)
                    .frame(height: 74)

                VStack(spacing: 3) {
                    PixelText(
                        text: "\(activePrayerType.displayName.uppercased()) · \(activePrayerTimeString)",
                        color: .white,
                        pixelSize: 1.6
                    )
                    PixelText(
                        text: statusSubtitle.uppercased(),
                        color: accent,
                        pixelSize: 1.05
                    )
                }
                .padding(.bottom, 10)
            }

            // Bottom 5-Prayer Baseline
            HStack(spacing: 4) {
                ForEach(prayerSlots) { slot in
                    let isActive = slot.type == activePrayerType
                    let timeText = slot.date.map { manager.formattedPrayerTime($0) } ?? "--:--"
                    VStack(spacing: 2) {
                        PixelText(
                            text: slot.shortCode,
                            color: isActive ? accent : Color.white.opacity(0.50),
                            pixelSize: 1.05
                        )
                        PixelText(
                            text: timeText,
                            color: isActive ? .white : Color.white.opacity(0.45),
                            pixelSize: 1.0
                        )
                    }
                    .frame(maxWidth: .infinity)
                }
            }
            .padding(.top, 2)
        }
        .padding(.horizontal, displayMode == .dynamicIsland ? 18 : 14)
        .padding(.vertical, displayMode == .dynamicIsland ? 11 : 9)
        .background(
            RadialGradient(
                colors: [accent.opacity(0.16), Color.clear],
                center: UnitPoint(x: 0.5, y: 0.12),
                startRadius: 4,
                endRadius: 210
            )
        )
    }

    // MARK: - 2. Modern Clean Compact Sun-Arc Banner

    private var modernBanner: some View {
        let skyCyan = Color(red: 0.49, green: 0.83, blue: 0.99)
        return VStack(spacing: 3) {
            // Top context row
            HStack {
                Text("Prayer · \(manager.cityName)")
                    .font(.system(size: 10.5, weight: .medium, design: .rounded))
                    .foregroundStyle(Color.white.opacity(0.56))
                    .lineLimit(1)
                Spacer(minLength: 8)
                Text(manager.currentTimeString)
                    .font(.system(size: 10.5, weight: .semibold, design: .rounded))
                    .monospacedDigit()
                    .foregroundStyle(Color.white.opacity(0.76))
            }

            // Horizon Sun-Arc + Centered Hero
            ZStack(alignment: .bottom) {
                modernSunArc
                    .frame(height: 74)

                VStack(spacing: 2) {
                    Text("\(activePrayerType.displayName) · \(activePrayerTimeString)")
                        .font(.system(size: 16, weight: .heavy, design: .rounded))
                        .foregroundStyle(.white)
                        .shadow(color: Color.black.opacity(0.9), radius: 6, y: 1)
                        .lineLimit(1)

                    Text(statusSubtitle)
                        .font(.system(size: 10.5, weight: .bold, design: .rounded))
                        .foregroundStyle(skyCyan)
                        .lineLimit(1)
                }
                .padding(.bottom, 9)
            }

            // Bottom 5-Prayer Baseline
            HStack(spacing: 2) {
                ForEach(prayerSlots) { slot in
                    let isActive = slot.type == activePrayerType
                    let timeText = slot.date.map { manager.formattedPrayerTime($0) } ?? "--:--"
                    VStack(spacing: 1) {
                        Text(slot.type.displayName)
                            .font(.system(size: 9.5, weight: isActive ? .bold : .medium, design: .rounded))
                            .foregroundStyle(isActive ? skyCyan : Color.white.opacity(0.52))
                            .lineLimit(1)
                        Text(timeText)
                            .font(.system(size: 9.5, weight: isActive ? .semibold : .regular, design: .rounded))
                            .monospacedDigit()
                            .foregroundStyle(isActive ? .white : Color.white.opacity(0.45))
                    }
                    .frame(maxWidth: .infinity)
                }
            }
            .padding(.top, 2)
        }
        .padding(.horizontal, displayMode == .dynamicIsland ? 18 : 14)
        .padding(.vertical, displayMode == .dynamicIsland ? 11 : 9)
        .background(
            RadialGradient(
                colors: [
                    Color(red: 0.22, green: 0.74, blue: 0.97).opacity(0.18),
                    Color(red: 0.15, green: 0.39, blue: 0.92).opacity(0.06),
                    Color.clear
                ],
                center: UnitPoint(x: 0.5, y: 0.10),
                startRadius: 4,
                endRadius: 220
            )
        )
    }

    // MARK: - Horizon Arc Renderers

    private func arcPoint(t: Double, width: CGFloat, height: CGFloat) -> CGPoint {
        let padX: CGFloat = 16
        let baseY = height - 6
        let peakY: CGFloat = 8
        let x = padX + CGFloat(t) * max(1, width - padX * 2)
        let y = baseY - CGFloat(sin(t * Double.pi)) * (baseY - peakY)
        return CGPoint(x: x, y: y)
    }

    private var modernSunArc: some View {
        let prog = arcProgress
        return Canvas { ctx, size in
            let w = size.width
            let h = size.height
            let baseY = h - 6

            // Horizon baseline
            var baseLine = Path()
            baseLine.move(to: CGPoint(x: 6, y: baseY))
            baseLine.addLine(to: CGPoint(x: w - 6, y: baseY))
            ctx.stroke(baseLine, with: .color(Color.white.opacity(0.12)), lineWidth: 1)

            // Future + past arc
            var fullArc = Path()
            var pastArc = Path()
            let segs = 48
            for i in 0...segs {
                let t = Double(i) / Double(segs)
                let pt = arcPoint(t: t, width: w, height: h)
                if i == 0 { fullArc.move(to: pt) } else { fullArc.addLine(to: pt) }
                if t <= prog {
                    if i == 0 { pastArc.move(to: pt) } else { pastArc.addLine(to: pt) }
                }
            }
            let sun = arcPoint(t: prog, width: w, height: h)
            if prog > 0 { pastArc.addLine(to: sun) }

            ctx.stroke(fullArc, with: .color(Color.white.opacity(0.16)), style: StrokeStyle(lineWidth: 2.2, lineCap: .round))
            ctx.stroke(
                pastArc,
                with: .linearGradient(
                    Gradient(colors: [Color(red: 0.22, green: 0.74, blue: 0.97), Color(red: 0.23, green: 0.51, blue: 0.96)]),
                    startPoint: CGPoint(x: 16, y: baseY),
                    endPoint: sun
                ),
                style: StrokeStyle(lineWidth: 2.6, lineCap: .round)
            )

            // 5 prayer nodes
            for (idx, slot) in prayerSlots.enumerated() {
                let t = slotProgress(slot, index: idx)
                let pt = arcPoint(t: t, width: w, height: h)
                let isActive = slot.type == activePrayerType
                let r: CGFloat = isActive ? 3.4 : 2.1
                let rect = CGRect(x: pt.x - r, y: pt.y - r, width: r * 2, height: r * 2)
                ctx.fill(Path(ellipseIn: rect), with: .color(isActive ? .white : Color.white.opacity(0.48)))
            }

            // Sun glow + core
            let glowRect = CGRect(x: sun.x - 10, y: sun.y - 10, width: 20, height: 20)
            ctx.fill(
                Path(ellipseIn: glowRect),
                with: .radialGradient(
                    Gradient(colors: [
                        Color.white,
                        Color(red: 0.99, green: 0.90, blue: 0.54).opacity(0.85),
                        Color(red: 0.96, green: 0.62, blue: 0.04).opacity(0.0)
                    ]),
                    center: sun,
                    startRadius: 0,
                    endRadius: 10
                )
            )
            ctx.fill(Path(ellipseIn: CGRect(x: sun.x - 3.5, y: sun.y - 3.5, width: 7, height: 7)), with: .color(.white))
        }
    }

    private func pixelSunArc(accent: Color) -> some View {
        let prog = arcProgress
        return Canvas { ctx, size in
            let w = size.width
            let h = size.height
            let baseY = h - 6
            let padX: CGFloat = 16
            let step: CGFloat = 2

            // Horizon baseline
            ctx.fill(Path(CGRect(x: 6, y: baseY, width: max(1, w - 12), height: 1.5)), with: .color(Color.white.opacity(0.14)))

            // Continuous stepped pixel curve
            var prevGy: CGFloat? = nil
            var x = padX
            while x <= w - padX {
                let t = Double((x - padX) / max(1, w - padX * 2))
                let rawPt = arcPoint(t: t, width: w, height: h)
                let gy = (rawPt.y / step).rounded() * step
                let col = t <= prog ? accent : Color.white.opacity(0.16)

                if let prev = prevGy, abs(gy - prev) > step {
                    let dir: CGFloat = gy > prev ? step : -step
                    var yy = prev + dir
                    while (dir > 0 && yy < gy) || (dir < 0 && yy > gy) {
                        ctx.fill(Path(CGRect(x: x, y: yy, width: 2, height: 2)), with: .color(col))
                        yy += dir
                    }
                }
                ctx.fill(Path(CGRect(x: x, y: gy, width: 2.2, height: 2.2)), with: .color(col))
                prevGy = gy
                x += step
            }

            // 5 pixel prayer nodes
            for (idx, slot) in prayerSlots.enumerated() {
                let t = slotProgress(slot, index: idx)
                let pt = arcPoint(t: t, width: w, height: h)
                let nx = (pt.x / 2).rounded() * 2
                let ny = (pt.y / 2).rounded() * 2
                let isActive = slot.type == activePrayerType
                ctx.fill(
                    Path(CGRect(x: nx - 2, y: ny - 2, width: 4, height: 4)),
                    with: .color(isActive ? accent : Color.white.opacity(0.48))
                )
            }

            // Multi-tone pixel sun orb
            let sun = arcPoint(t: prog, width: w, height: h)
            let sx = (sun.x / 2).rounded() * 2
            let sy = (sun.y / 2).rounded() * 2
            ctx.fill(Path(CGRect(x: sx - 5, y: sy - 3, width: 10, height: 6)), with: .color(Color(red: 0.99, green: 0.88, blue: 0.28).opacity(0.24)))
            ctx.fill(Path(CGRect(x: sx - 3, y: sy - 5, width: 6, height: 10)), with: .color(Color(red: 0.99, green: 0.88, blue: 0.28).opacity(0.24)))
            ctx.fill(Path(CGRect(x: sx - 3, y: sy - 2, width: 6, height: 4)), with: .color(Color(red: 0.98, green: 0.75, blue: 0.14)))
            ctx.fill(Path(CGRect(x: sx - 2, y: sy - 3, width: 4, height: 6)), with: .color(Color(red: 0.98, green: 0.75, blue: 0.14)))
            ctx.fill(Path(CGRect(x: sx - 1.5, y: sy - 1.5, width: 3, height: 3)), with: .color(Color(red: 1.0, green: 0.98, blue: 0.92)))
        }
    }
}
