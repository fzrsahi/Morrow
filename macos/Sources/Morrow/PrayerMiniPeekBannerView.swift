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

    @State private var animatedArcProgress: Double = 0.0
    @State private var celestialPulse: Bool = false

    private var isPixel: Bool {
        islandUITheme == IslandUITheme.pixel.rawValue
    }

    private var prayerAccent: Color {
        if (syncThemeWithAlbumArt || musicHUDProgressColor == "album") && nowPlaying.isPlaying && nowPlaying.artworkImage != nil {
            return AppColorTheme.readableOnDark(nowPlaying.albumPalette.primary)
        }
        return AppColorTheme.readableOnDark(
            AppColorTheme.accent(for: musicHUDProgressColor, isModern: !isPixel)
        )
    }

    public init(manager: PrayerTimeManager, displayMode: IslandDisplayMode = .notch, onOpenFull: @escaping () -> Void) {
        self.manager = manager
        self.displayMode = displayMode
        self.onOpenFull = onOpenFull
    }

    private var activePrayerType: PrayerType {
        if let preview = manager.previewPrayerType {
            return preview
        }
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

    private var isNightMoon: Bool {
        activePrayerType == .isya
    }

    private var isHalfSunHorizon: Bool {
        activePrayerType == .subuh || activePrayerType == .maghrib
    }

    private var orbProgress: Double {
        switch activePrayerType {
        case .subuh: return 0.04
        case .maghrib: return 0.96
        case .isya: return 0.50
        default: return arcProgress
        }
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
        if let preview = manager.previewPrayerType,
           let idx = prayerSlots.firstIndex(where: { $0.type == preview }) {
            return slotProgress(prayerSlots[idx], index: idx)
        }
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
        .onAppear {
            startEntryAnimation()
        }
        .onChange(of: activePrayerType) { _, _ in
            startEntryAnimation()
        }
    }

    private func startEntryAnimation() {
        let target = orbProgress
        animatedArcProgress = max(0.0, target - 0.18)
        celestialPulse = false
        withAnimation(.easeOut(duration: 0.65)) {
            animatedArcProgress = target
        }
        withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) {
            celestialPulse = true
        }
    }

    // MARK: - 1. Pixel 32-Bit Compact Sun-Arc Banner

    private var pixelBanner: some View {
        let accent = prayerAccent
        return VStack(spacing: 3) {
            // Top context row
            HStack {
                PixelText(
                    text: "PRAYER · \(manager.cityName.uppercased())",
                    color: Color.white.opacity(0.62),
                    pixelSize: 1.05
                )
                .layoutPriority(0)
                Spacer(minLength: 4)
                PixelText(
                    text: "NOW",
                    color: accent,
                    pixelSize: 1.05
                )
                .fixedSize()
                .layoutPriority(2)
            }

            // Horizon Sun/Moon-Arc + Centered Hero
            ZStack(alignment: .bottom) {
                pixelSunArc(accent: accent)
                    .frame(height: 84)

                VStack(spacing: 3) {
                    PixelText(
                        text: "\(activePrayerType.displayName.uppercased()) · \(activePrayerTimeString)",
                        color: .white,
                        pixelSize: 1.55
                    )
                    PixelText(
                        text: "TIME FOR PRAYER",
                        color: accent.opacity(0.92),
                        pixelSize: 1.0
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
    }

    // MARK: - 2. Modern Clean Compact Sun-Arc Banner

    private var modernBanner: some View {
        let accent = prayerAccent
        return VStack(spacing: 3) {
            // Top context row
            HStack {
                Text("Prayer · \(manager.cityName)")
                    .font(.system(size: 10.5, weight: .medium, design: .rounded))
                    .foregroundStyle(Color.white.opacity(0.62))
                    .lineLimit(1)
                    .minimumScaleFactor(0.85)
                    .layoutPriority(0)
                Spacer(minLength: 4)
                Text("NOW")
                    .font(.system(size: 10, weight: .bold, design: .rounded))
                    .tracking(0.7)
                    .foregroundStyle(accent)
                    .fixedSize()
                    .layoutPriority(2)
            }

            // Horizon Sun/Moon-Arc + Centered Hero
            ZStack(alignment: .bottom) {
                modernSunArc(accent: accent)
                    .frame(height: 84)

                VStack(spacing: 2) {
                    Text("\(activePrayerType.displayName) · \(activePrayerTimeString)")
                        .font(.system(size: 15.5, weight: .heavy, design: .rounded))
                        .foregroundStyle(.white)
                        .shadow(color: Color.black.opacity(0.9), radius: 6, y: 1)
                        .lineLimit(1)

                    Text("Time for prayer")
                        .font(.system(size: 10.5, weight: .semibold, design: .rounded))
                        .foregroundStyle(accent.opacity(0.92))
                        .lineLimit(1)
                }
                .padding(.bottom, 10)
            }

            // Bottom 5-Prayer Baseline
            HStack(spacing: 2) {
                ForEach(prayerSlots) { slot in
                    let isActive = slot.type == activePrayerType
                    let timeText = slot.date.map { manager.formattedPrayerTime($0) } ?? "--:--"
                    VStack(spacing: 1) {
                        Text(slot.type.displayName)
                            .font(.system(size: 9.5, weight: isActive ? .bold : .medium, design: .rounded))
                            .foregroundStyle(isActive ? accent : Color.white.opacity(0.52))
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

    private static let pixelSunMatrix: [[Int]] = [
        [0,0,0,0,0,1,0,0,0,0,0],
        [0,1,0,0,0,1,0,0,0,1,0],
        [0,0,1,0,1,1,1,0,1,0,0],
        [0,0,0,1,1,1,1,1,0,0,0],
        [0,0,1,1,1,1,1,1,1,0,0],
        [1,1,1,1,1,1,1,1,1,1,1],
        [0,0,1,1,1,1,1,1,1,0,0],
        [0,0,0,1,1,1,1,1,0,0,0],
        [0,0,1,0,1,1,1,0,1,0,0],
        [0,1,0,0,0,1,0,0,0,1,0],
        [0,0,0,0,0,1,0,0,0,0,0]
    ]

    // 12x12 crisp 2-tone 16-bit Crescent Moon (1 = warm highlight, 2 = gold shadow)
    private static let pixelMoonMatrix: [[Int]] = [
        [0,0,0,0,0,1,1,2,0,0,0,0],
        [0,0,0,1,1,1,1,0,0,0,0,0],
        [0,0,1,1,1,2,0,0,0,0,0,0],
        [0,1,1,1,2,0,0,0,0,0,0,0],
        [0,1,1,2,0,0,0,0,0,0,0,0],
        [1,1,1,2,0,0,0,0,0,0,0,0],
        [1,1,1,2,0,0,0,0,0,0,0,0],
        [0,1,1,2,0,0,0,0,0,0,0,0],
        [0,1,1,1,2,0,0,0,0,0,0,0],
        [0,0,1,1,1,2,0,0,0,0,0,0],
        [0,0,0,1,1,1,1,0,0,0,0,0],
        [0,0,0,0,0,1,1,2,0,0,0,0]
    ]

    // 9x4 pixel half-sun dome above sea horizon
    private static let pixelHalfSunMatrix: [[Int]] = [
        [0,0,1,0,1,0,1,0,0],
        [0,0,0,1,1,1,0,0,0],
        [0,1,1,1,1,1,1,1,0],
        [1,1,1,1,1,1,1,1,1]
    ]

    private func modernSunArc(accent: Color) -> some View {
        let prog = animatedArcProgress
        let isNight = isNightMoon
        let isHalfSun = isHalfSunHorizon
        let isRise = activePrayerType == .subuh
        let haloAlpha = celestialPulse ? 0.34 : 0.16
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
            let orb = arcPoint(t: prog, width: w, height: h)
            if prog > 0 { pastArc.addLine(to: orb) }

            ctx.stroke(fullArc, with: .color(Color.white.opacity(0.16)), style: StrokeStyle(lineWidth: 2.2, lineCap: .round))
            ctx.stroke(
                pastArc,
                with: .color(accent),
                style: StrokeStyle(lineWidth: 2.4, lineCap: .round)
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

            if isNight {
                for starPt in [CGPoint(x: w * 0.19, y: 26), CGPoint(x: w * 0.81, y: 22), CGPoint(x: w * 0.29, y: 16)] {
                    ctx.fill(Path(ellipseIn: CGRect(x: starPt.x - 1.2, y: starPt.y - 1.2, width: 2.4, height: 2.4)), with: .color(Color.white.opacity(0.55)))
                }
            }

            if isHalfSun {
                let targetIdx = isRise ? 0 : 3
                let targetT = slotProgress(prayerSlots[targetIdx], index: targetIdx)
                let sunPt = arcPoint(t: targetT, width: w, height: h)
                let sunX = sunPt.x
                let sunY = baseY
                let sunCol = isRise ? Color(red: 0.99, green: 0.90, blue: 0.54) : Color(red: 0.98, green: 0.57, blue: 0.24)
                let seaCol = isRise ? Color(red: 0.22, green: 0.74, blue: 0.97) : Color(red: 0.98, green: 0.57, blue: 0.24)
                var dome = Path()
                dome.addArc(center: CGPoint(x: sunX, y: sunY), radius: 7, startAngle: .degrees(180), endAngle: .degrees(0), clockwise: false)
                dome.closeSubpath()
                ctx.fill(dome, with: .color(sunCol))

                for (dx, dy, op) in [(9.0, 2.5, 0.6), (5.0, 5.0, 0.35)] {
                    var ripple = Path()
                    ripple.move(to: CGPoint(x: sunX - dx, y: sunY + dy))
                    ripple.addLine(to: CGPoint(x: sunX + dx, y: sunY + dy))
                    ctx.stroke(ripple, with: .color(seaCol.opacity(op)), style: StrokeStyle(lineWidth: 1.3, lineCap: .round))
                }
            } else {
                // Smooth radial halo (no hard dark target circle)
                ctx.fill(
                    Path(ellipseIn: CGRect(x: orb.x - 14, y: orb.y - 14, width: 28, height: 28)),
                    with: .radialGradient(
                        Gradient(colors: [accent.opacity(haloAlpha), Color.clear]),
                        center: orb,
                        startRadius: 2,
                        endRadius: 14
                    )
                )
                if isNight {
                    let moonColor = Color(red: 0.99, green: 0.94, blue: 0.54)
                    var moonPath = Path()
                    moonPath.addArc(center: orb, radius: 6.8, startAngle: .degrees(-65), endAngle: .degrees(65), clockwise: true)
                    moonPath.addArc(center: CGPoint(x: orb.x + 2.8, y: orb.y), radius: 5.4, startAngle: .degrees(72), endAngle: .degrees(-72), clockwise: false)
                    moonPath.closeSubpath()
                    ctx.fill(moonPath, with: .color(moonColor))
                } else {
                    let coreColor = Color(red: 1.0, green: 0.82, blue: 0.40)
                    for i in 0..<8 {
                        let a = Double(i) * Double.pi / 4.0
                        var ray = Path()
                        ray.move(to: CGPoint(x: orb.x + cos(a) * 6.0, y: orb.y + sin(a) * 6.0))
                        ray.addLine(to: CGPoint(x: orb.x + cos(a) * 9.0, y: orb.y + sin(a) * 9.0))
                        ctx.stroke(ray, with: .color(coreColor), style: StrokeStyle(lineWidth: 1.4, lineCap: .round))
                    }
                    ctx.fill(Path(ellipseIn: CGRect(x: orb.x - 4.2, y: orb.y - 4.2, width: 8.4, height: 8.4)), with: .color(coreColor))
                }
            }
        }
    }

    private func pixelSunArc(accent: Color) -> some View {
        let prog = animatedArcProgress
        let isNight = isNightMoon
        let isHalfSun = isHalfSunHorizon
        let isRise = activePrayerType == .subuh
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

            if isNight {
                for starPt in [CGPoint(x: w * 0.19, y: 26), CGPoint(x: w * 0.81, y: 22), CGPoint(x: w * 0.29, y: 16)] {
                    ctx.fill(Path(CGRect(x: (starPt.x / 2).rounded() * 2, y: (starPt.y / 2).rounded() * 2, width: 2, height: 2)), with: .color(Color.white.opacity(0.50)))
                }
            }

            if isHalfSun {
                let targetIdx = isRise ? 0 : 3
                let targetT = slotProgress(prayerSlots[targetIdx], index: targetIdx)
                let sunPt = arcPoint(t: targetT, width: w, height: h)
                let sunX = (sunPt.x / 2).rounded() * 2
                let sunCol = isRise ? Color(red: 0.99, green: 0.90, blue: 0.54) : Color(red: 0.98, green: 0.57, blue: 0.24)
                let seaCol = isRise ? Color(red: 0.22, green: 0.74, blue: 0.97) : Color(red: 0.98, green: 0.57, blue: 0.24)
                let px: CGFloat = 2
                let ox = sunX - (CGFloat(Self.pixelHalfSunMatrix[0].count) * px) / 2
                let oy = baseY - CGFloat(Self.pixelHalfSunMatrix.count) * px
                for (r, row) in Self.pixelHalfSunMatrix.enumerated() {
                    for (c, bit) in row.enumerated() where bit == 1 {
                        ctx.fill(
                            Path(CGRect(x: ox + CGFloat(c) * px, y: oy + CGFloat(r) * px, width: px, height: px)),
                            with: .color(sunCol)
                        )
                    }
                }
                ctx.fill(Path(CGRect(x: sunX - 8, y: baseY + 2, width: 16, height: 2)), with: .color(seaCol.opacity(0.55)))
                ctx.fill(Path(CGRect(x: sunX - 5, y: baseY + 5, width: 10, height: 2)), with: .color(seaCol.opacity(0.32)))
            } else {
                let orb = arcPoint(t: prog, width: w, height: h)
                let sx = (orb.x / 2).rounded() * 2
                let sy = (orb.y / 2).rounded() * 2
                ctx.fill(Path(CGRect(x: sx - 13, y: sy - 13, width: 26, height: 26)), with: .color(Color.black.opacity(0.72)))
                let matrix = isNight ? Self.pixelMoonMatrix : Self.pixelSunMatrix
                let px: CGFloat = 2
                let ox = sx - (CGFloat(matrix[0].count) * px) / 2 + (isNight ? 2 : 0)
                let oy = sy - (CGFloat(matrix.count) * px) / 2
                for (r, row) in matrix.enumerated() {
                    for (c, bit) in row.enumerated() where bit > 0 {
                        let col: Color = isNight
                            ? (bit == 1 ? Color(red: 0.99, green: 0.94, blue: 0.54) : Color(red: 0.96, green: 0.62, blue: 0.04))
                            : Color(red: 1.0, green: 0.82, blue: 0.40)
                        ctx.fill(
                            Path(CGRect(x: ox + CGFloat(c) * px, y: oy + CGFloat(r) * px, width: px, height: px)),
                            with: .color(col)
                        )
                    }
                }
            }
        }
    }
}
