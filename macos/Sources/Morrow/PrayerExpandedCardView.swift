import SwiftUI
import MorrowCore

/// Unified living diorama prayer card with an integrated 3-state timeline and Imsak/Sunrise horology.
/// Fully supports both Compact (120pt) and Panoramic (242pt) display modes, across 16-bit Pixel and Modern themes.
public struct PrayerExpandedCardView: View {
    @ObservedObject public var manager: PrayerTimeManager

    @AppStorage(SettingsKey.islandUITheme) private var islandUITheme = SettingsDefaults.islandUITheme
    @AppStorage(SettingsKey.prayerCardDisplayMode) private var prayerCardDisplayMode = SettingsDefaults.prayerCardDisplayMode
    @State private var hoveredPrayer: String? = nil

    private var isPixel: Bool {
        islandUITheme == IslandUITheme.pixel.rawValue
    }

    private var isPanoramic: Bool {
        prayerCardDisplayMode == PrayerCardDisplayMode.panoramic.rawValue
    }

    @AppStorage(SettingsKey.musicHUDProgressColor) private var musicHUDProgressColor = SettingsDefaults.musicHUDProgressColor
    @AppStorage(SettingsKey.syncThemeWithAlbumArt) private var syncThemeWithAlbumArt = SettingsDefaults.syncThemeWithAlbumArt
    @ObservedObject private var nowPlaying = NowPlayingManager.shared

    private struct SolarPhaseStyle {
        let label: String
        let icon: String
        let accent: Color

        static func forHour(_ hour: Double, isPixel: Bool) -> SolarPhaseStyle {
            let h = hour.truncatingRemainder(dividingBy: 24)
            let rawLabel: String
            let icon: String
            let accent: Color

            if h >= 4.4 && h < 6.5 {
                rawLabel = "Dawn"
                icon = "sun.horizon.fill"
                accent = Color(red: 1.0, green: 0.70, blue: 0.40)
            } else if h >= 6.5 && h < 15.0 {
                rawLabel = "Midday"
                icon = "sun.max.fill"
                accent = Color(red: 1.0, green: 0.88, blue: 0.40)
            } else if h >= 15.0 && h < 17.4 {
                rawLabel = "Afternoon"
                icon = "sun.haze.fill"
                accent = Color(red: 1.0, green: 0.75, blue: 0.26)
            } else if h >= 17.4 && h < 19.1 {
                rawLabel = "Sunset"
                icon = "sun.and.horizon.fill"
                accent = Color(red: 1.0, green: 0.70, blue: 0.28)
            } else {
                rawLabel = "Night"
                icon = "moon.stars.fill"
                accent = Color(red: 0.18, green: 0.91, blue: 0.78)
            }
            return SolarPhaseStyle(
                label: isPixel ? rawLabel.uppercased() : rawLabel,
                icon: icon,
                accent: accent
            )
        }
    }

    private var solarPhase: SolarPhaseStyle {
        SolarPhaseStyle.forHour(manager.currentFractionalHour, isPixel: isPixel)
    }

    private var accentColor: Color {
        if (syncThemeWithAlbumArt || musicHUDProgressColor == "album") && nowPlaying.isPlaying && nowPlaying.artworkImage != nil {
            return nowPlaying.albumPalette.primary
        }
        let pc = MusicProgressColor(rawValue: musicHUDProgressColor) ?? .theme
        switch pc {
        case .album:
            if nowPlaying.isPlaying && nowPlaying.artworkImage != nil {
                return nowPlaying.albumPalette.primary
            }
            return solarPhase.accent
        case .theme, .cyan: return solarPhase.accent
        case .white: return .white
        case .pink: return Color(red: 1.0, green: 0.16, blue: 0.53)
        case .green: return Color(red: 0.12, green: 0.84, blue: 0.38)
        case .amber: return Color(red: 1.0, green: 0.72, blue: 0.20)
        }
    }

    private var fontDesign: Font.Design {
        isPixel ? .monospaced : .rounded
    }

    private var slotFill: Color {
        isPixel ? Pixel16Palette.slot.opacity(0.68) : Color.black.opacity(0.42)
    }

    public init(manager: PrayerTimeManager) {
        self.manager = manager
    }

    /// `HH:mm:ss` in the city's zone at a given instant.
    static func clockText(at date: Date, timeZone: TimeZone) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeZone
        let c = calendar.dateComponents([.hour, .minute, .second], from: date)
        return String(format: "%02d:%02d:%02d", c.hour ?? 0, c.minute ?? 0, c.second ?? 0)
    }

    /// Full weekday + day + month name (e.g. `Monday, 23 November`) in the city's time zone.
    static func fullGregorianDateText(at date: Date, timeZone: TimeZone) -> String {
        let df = DateFormatter()
        df.locale = Locale(identifier: "en_US_POSIX")
        df.timeZone = timeZone
        df.dateFormat = "EEEE, d MMMM"
        return df.string(from: date)
    }

    private func themedText(_ text: String) -> String {
        isPixel ? text.uppercased() : text
    }

    public var body: some View {
        Group {
            if isPanoramic {
                panoramicDashboard
            } else {
                compactDashboard
            }
        }
        .padding(.horizontal, 18)
        .padding(.top, 4)
        .padding(.bottom, 16)
        .frame(maxWidth: .infinity)
    }

    // MARK: - 1. PANORAMIC DASHBOARD (242pt)

    private var panoramicDashboard: some View {
        let upcoming = manager.upcomingPrayer
        let schedule = manager.todaySchedule
        let phase = solarPhase
        let now = manager.simulatedDate ?? Date()
        let gregorianFull = themedText(Self.fullGregorianDateText(at: now, timeZone: manager.timeZone))

        return ZStack(alignment: .topLeading) {
            DioramaSkyView(
                hour: manager.currentFractionalHour,
                hijriDay: manager.hijriDay,
                sunriseHour: manager.sunriseHour,
                sunsetHour: manager.sunsetHour,
                pixel: isPixel,
                skyline: true,
                skylineHeight: 0.18
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))

            // Seamless atmospheric legibility gradient
            LinearGradient(
                colors: [
                    Color.black.opacity(0.14),
                    Color.black.opacity(0.32),
                    Color.black.opacity(0.88)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .allowsHitTesting(false)

            VStack(spacing: 0) {
                // Zone 1: Content-hugging Top Header (Phase + Lunar Hijri Date on left, Map Pin City on right)
                HStack(alignment: .center, spacing: 10) {
                    phaseChip(phase: phase)

                    Text(themedText(manager.hijriDateString))
                        .font(.system(size: 8.5, weight: .semibold, design: fontDesign))
                        .foregroundStyle(Color.white.opacity(0.82))
                        .lineLimit(1)

                    Spacer(minLength: 8)

                    cityChip
                }
                .padding(.horizontal, 14)
                .padding(.top, 12)

                Spacer(minLength: 6)

                // Zone 2: Left Horology Stack & Right Imsak/Sunrise Card
                HStack(alignment: .center, spacing: 14) {
                    VStack(alignment: .leading, spacing: 3) {
                        if let next = upcoming {
                            let diffMins = manager.minutesUntilNextPrayer
                            HStack(spacing: 6) {
                                Text(themedText(next.prayer.displayName))
                                    .font(.system(size: 14.5, weight: .bold, design: fontDesign))
                                    .foregroundStyle(Color.white.opacity(0.82))

                                Text(manager.formattedPrayerTime(next.date))
                                    .font(.system(size: 14.5, weight: .bold, design: fontDesign))
                                    .foregroundStyle(accentColor)

                                Text(diffMins <= 0 ? (isPixel ? "(NOW)" : "(Now)") : "(-\(manager.formattedCountdown))")
                                    .font(.system(size: 9.5, weight: .bold, design: fontDesign))
                                    .foregroundStyle(Color(red: 0.73, green: 0.77, blue: 0.84))
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(linelessSlotShape(cornerRadius: 3))
                            }
                        }

                        TimelineView(.periodic(from: .now, by: 1)) { context in
                            Text(Self.clockText(at: context.date, timeZone: manager.timeZone))
                                .font(.system(size: 26, weight: .black, design: fontDesign))
                                .foregroundStyle(Color.white)
                                .shadow(color: Color.black.opacity(0.75), radius: 3)
                        }

                        Text(gregorianFull)
                            .font(.system(size: 8.5, weight: .medium, design: fontDesign))
                            .foregroundStyle(Color.white.opacity(0.52))
                            .lineLimit(1)
                    }

                    Spacer()

                    if let sched = schedule {
                        milestonesCard(sched: sched)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 4)

                Spacer(minLength: 6)

                // Zone 3: Segmented 3-State Timeline Ribbon
                if let sched = schedule {
                    timelineRibbon(sched: sched)
                        .padding(.horizontal, 10)
                        .padding(.bottom, 8)
                }
            }
        }
        .frame(height: 242)
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    // MARK: - 2. COMPACT RIBBON (120pt)

    private var compactDashboard: some View {
        let upcoming = manager.upcomingPrayer
        let schedule = manager.todaySchedule
        let phase = solarPhase

        return ZStack(alignment: .topLeading) {
            DioramaSkyView(
                hour: manager.currentFractionalHour,
                hijriDay: manager.hijriDay,
                sunriseHour: manager.sunriseHour,
                sunsetHour: manager.sunsetHour,
                pixel: isPixel,
                skyline: true,
                skylineHeight: 0.28
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))

            LinearGradient(
                colors: [
                    Color.black.opacity(0.18),
                    Color.black.opacity(0.46),
                    Color.black.opacity(0.90)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .allowsHitTesting(false)

            VStack(spacing: 6) {
                HStack(alignment: .center, spacing: 7) {
                    phaseChip(phase: phase)

                    Text(themedText(manager.hijriDateString))
                        .font(.system(size: 8, weight: .semibold, design: fontDesign))
                        .foregroundStyle(Color.white.opacity(0.85))
                        .lineLimit(1)

                    Spacer(minLength: 4)

                    if let next = upcoming {
                        let diffMins = manager.minutesUntilNextPrayer
                        HStack(spacing: 5) {
                            Text(themedText(next.prayer.displayName))
                                .font(.system(size: 8.5, weight: .bold, design: fontDesign))
                                .foregroundStyle(Color.white.opacity(0.85))

                            Text(manager.formattedPrayerTime(next.date))
                                .font(.system(size: 8.5, weight: .bold, design: fontDesign))
                                .foregroundStyle(accentColor)

                            Text(diffMins <= 0 ? (isPixel ? "(NOW)" : "(Now)") : "(-\(manager.formattedCountdown))")
                                .font(.system(size: 7.5, weight: .bold, design: fontDesign))
                                .foregroundStyle(Color(red: 0.73, green: 0.77, blue: 0.84))
                        }
                        .padding(.horizontal, 7)
                        .padding(.vertical, 3)
                        .background(linelessSlotShape(cornerRadius: 3.5))
                    }

                    if let sched = schedule {
                        Text("\(themedText("Imsak")) \(manager.formattedPrayerTime(sched.imsak))")
                            .font(.system(size: 7.5, weight: .bold, design: fontDesign))
                            .foregroundStyle(Color.white.opacity(0.70))
                            .padding(.horizontal, 7)
                            .padding(.vertical, 3)
                            .background(linelessSlotShape(cornerRadius: 3.5))
                    }

                    cityChip
                }
                .padding(.horizontal, 14)
                .padding(.top, 8)

                Spacer(minLength: 2)

                if let sched = schedule {
                    timelineRibbon(sched: sched)
                        .padding(.horizontal, 10)
                        .padding(.bottom, 6)
                }
            }
        }
        .frame(height: 120)
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    // MARK: - 3. SHARED LINELESS COMPONENTS

    private func phaseChip(phase: SolarPhaseStyle) -> some View {
        HStack(spacing: 5) {
            Image(systemName: phase.icon)
                .font(.system(size: 8.5, weight: .bold))
                .foregroundStyle(accentColor)
            Text(phase.label)
                .font(.system(size: 8, weight: .bold, design: fontDesign))
                .foregroundStyle(Color.white)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(linelessSlotShape(cornerRadius: 3.5))
        .fixedSize()
    }

    private var cityChip: some View {
        HStack(spacing: 4.5) {
            Image(systemName: "mappin.and.ellipse")
                .font(.system(size: 8.5, weight: .bold))
                .foregroundStyle(accentColor)
            Text(themedText(manager.cityName))
                .font(.system(size: 8, weight: .bold, design: fontDesign))
                .foregroundStyle(Color.white.opacity(0.95))
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .background(linelessSlotShape(cornerRadius: 3.5))
        .fixedSize()
    }

    private func milestonesCard(sched: DailyPrayerSchedule) -> some View {
        VStack(spacing: 6) {
            HStack {
                Text(themedText("Imsak"))
                    .font(.system(size: 8, weight: .bold, design: fontDesign))
                    .foregroundStyle(Color.white.opacity(0.52))
                Spacer()
                Text(manager.formattedPrayerTime(sched.imsak))
                    .font(.system(size: 10, weight: .bold, design: fontDesign))
                    .foregroundStyle(Color.white)
            }

            HStack {
                Text(themedText("Sunrise"))
                    .font(.system(size: 8, weight: .bold, design: fontDesign))
                    .foregroundStyle(Color.white.opacity(0.52))
                Spacer()
                Text(manager.formattedPrayerTime(sched.terbit))
                    .font(.system(size: 10, weight: .bold, design: fontDesign))
                    .foregroundStyle(Color(red: 1.0, green: 0.80, blue: 0.36))
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .frame(width: 140)
        .background(linelessSlotShape(cornerRadius: 4, modernRadius: 12))
    }

    @ViewBuilder
    private func linelessSlotShape(cornerRadius: CGFloat, modernRadius: CGFloat? = nil) -> some View {
        if isPixel {
            RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                .fill(slotFill)
        } else if let modernRadius {
            RoundedRectangle(cornerRadius: modernRadius, style: .continuous)
                .fill(slotFill)
        } else {
            Capsule()
                .fill(slotFill)
        }
    }

    // MARK: - 4. SEGMENTED 3-STATE TIMELINE RIBBON

    @ViewBuilder
    private func timelineRibbon(sched: DailyPrayerSchedule) -> some View {
        let now = manager.simulatedDate ?? Date()
        let upcoming = manager.upcomingPrayer
        let prayers = sched.fivePrayers
        let currentPrayer = sched.currentFardhPrayer(at: now)
        let currentIndex = prayers.firstIndex { $0.prayer == currentPrayer } ?? -1
        let stationCount = prayers.count

        ZStack(alignment: .top) {
            // Segmented horizontal rail aligned strictly between node beacons (never cuts labels)
            GeometryReader { geo in
                let spacing: CGFloat = 4
                let colW = (geo.size.width - spacing * CGFloat(max(0, stationCount - 1))) / CGFloat(max(1, stationCount))
                ForEach(0..<max(0, stationCount - 1), id: \.self) { i in
                    let cx1 = CGFloat(i) * (colW + spacing) + colW / 2
                    let cx2 = CGFloat(i + 1) * (colW + spacing) + colW / 2
                    let segX = cx1 + 12
                    let segW = max(0, (cx2 - 12) - segX)
                    let isCompleted = i < currentIndex
                    Rectangle()
                        .fill(isCompleted ? accentColor.opacity(0.55) : Color.white.opacity(0.12))
                        .frame(width: segW, height: 1.5)
                        .position(x: segX + segW / 2, y: 6)
                }
            }
            .frame(height: 12)
            .allowsHitTesting(false)

            // 5 Prayer Stations (FAJR, DHUHR, ASR, MAGHRIB, ISHA)
            HStack(spacing: 4) {
                ForEach(Array(prayers.enumerated()), id: \.element.prayer.rawValue) { _, item in
                    let isNext = (item.prayer == upcoming?.prayer)
                    let isCurrent = (item.prayer == currentPrayer && !isNext)
                    let isPassed = (!isNext && !isCurrent && item.date < now)
                    let isHovered = (hoveredPrayer == item.prayer.rawValue)

                    VStack(spacing: 2.5) {
                        // 1. Station Node Beacon
                        ZStack {
                            if isNext {
                                Circle()
                                    .fill(accentColor)
                                    .frame(width: 10, height: 10)
                                Circle()
                                    .fill(Color.black)
                                    .frame(width: 4, height: 4)
                            } else if isCurrent {
                                Circle()
                                    .stroke(Color.white.opacity(0.85), lineWidth: 1.5)
                                    .frame(width: 9, height: 9)
                                Circle()
                                    .fill(Color.white)
                                    .frame(width: 4, height: 4)
                            } else if isPassed {
                                Circle()
                                    .fill(accentColor.opacity(0.25))
                                    .frame(width: 9, height: 9)
                                Image(systemName: "checkmark")
                                    .font(.system(size: 4.5, weight: .black))
                                    .foregroundStyle(accentColor)
                            } else {
                                Circle()
                                    .fill(Color.white.opacity(0.25))
                                    .frame(width: 6, height: 6)
                            }
                        }
                        .frame(height: 12)

                        // 2. Status Tag (Unified accent role for NEXT; subtle translucent pill for CURRENT)
                        if isNext {
                            statusTag(text: "NEXT", foreground: .black, background: accentColor)
                        } else if isCurrent {
                            statusTag(text: "CURRENT", foreground: Color.white.opacity(0.92), background: Color.white.opacity(0.13))
                        } else {
                            Text(isPassed ? "PASSED" : "UPCOMING")
                                .font(.system(size: 6.5, weight: .semibold, design: fontDesign))
                                .foregroundStyle(Color.white.opacity(isPassed ? 0.34 : 0.42))
                                .frame(height: 11)
                        }

                        // 3. Prayer Name
                        Text(item.prayer.displayName.uppercased())
                            .font(.system(size: 8.5, weight: (isNext || isCurrent) ? .heavy : .bold, design: fontDesign))
                            .foregroundStyle(
                                isNext ? accentColor :
                                (isCurrent ? Color.white :
                                (isPassed ? Color.white.opacity(0.45) : Color.white.opacity(0.82)))
                            )

                        // 4. Formatted Time
                        Text(manager.formattedPrayerTime(item.date))
                            .font(.system(size: 11, weight: isNext ? .black : .bold, design: fontDesign))
                            .foregroundStyle(
                                isNext ? accentColor :
                                (isCurrent ? Color.white :
                                (isPassed ? Color.white.opacity(0.48) : Color.white))
                            )
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 1)
                    .background(
                        Group {
                            if isHovered {
                                RoundedRectangle(cornerRadius: 6)
                                    .fill(Color.white.opacity(0.08))
                            } else {
                                Color.clear
                            }
                        }
                    )
                    .contentShape(Rectangle())
                    .onHover { hoveredPrayer = $0 ? item.prayer.rawValue : nil }
                    .help("Prayer time for \(item.prayer.displayName): \(manager.formattedPrayerTime(item.date))")
                }
            }
        }
        .padding(.vertical, 2)
    }

    @ViewBuilder
    private func statusTag(text: String, foreground: Color, background: Color) -> some View {
        Text(text)
            .font(.system(size: 6.5, weight: .black, design: fontDesign))
            .foregroundStyle(foreground)
            .padding(.horizontal, 5)
            .frame(height: 11)
            .background(
                Group {
                    if isPixel {
                        RoundedRectangle(cornerRadius: 1.5, style: .continuous).fill(background)
                    } else {
                        Capsule().fill(background)
                    }
                }
            )
    }
}
