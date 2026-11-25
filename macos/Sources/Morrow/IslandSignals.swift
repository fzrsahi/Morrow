import Foundation
import Observation
import SwiftUI

/// The values the collapsed bar actually shows.
///
/// The panel root and both wings used to observe five singletons directly, so
/// every published property on any of them — a track position four times a
/// second, a CPU sample, a workspace refresh — re-rendered the whole bar even
/// when nothing visible had changed. This is the short list of values the bar
/// reads; the managers write to it, and a write that does not change the value
/// does not notify anyone.
@Observable
final class IslandSignals {
    static let shared = IslandSignals()

    // Music
    private(set) var isPlaying = false
    private(set) var showToast = false
    private(set) var hasTitle = false
    private(set) var hasLyrics = false
    /// The track name, for the one `.onChange` that opens the Music tab when a
    /// new track starts.
    private(set) var musicTitle = ""
    private(set) var currentLyricText = ""
    private(set) var currentLyricIndex = 0
    private(set) var lyricsLoading = false
    /// The collapsed bar's music tooltip, composed once per track instead of
    /// on every render.
    private(set) var musicTooltip = "Session & Token Usage"
    private(set) var albumPaletteRevision: Int = 0
    private(set) var albumPalette: AlbumArtPalette = .fallback
    private(set) var hasAlbumArtwork: Bool = false

    // Session / panel
    private(set) var tooltipText = ""
    private(set) var isDetailExpanded = false

    // Banners and HUDs
    private(set) var showChargingBanner = false
    private(set) var batteryLevel = 100
    private(set) var chargingBannerState: PowerBannerState = .charging(kind: .magSafe, watts: nil, timeToFullMinutes: nil)
    private(set) var showMiniPeekBanner = false
    private(set) var isVolumeHUDVisible = false

    // Prayer
    private(set) var isPrayerPrioritized = false

    // Sound
    private(set) var inQuietHours = false

    init() {}

    // MARK: - Writes
    //
    // Each setter compares before assigning: with `@Observable`, an assignment
    // is a notification whether or not the value moved.

    func setMusic(isPlaying: Bool, showToast: Bool, hasTitle: Bool, hasLyrics: Bool,
                  title: String, artist: String) {
        if self.isPlaying != isPlaying { self.isPlaying = isPlaying }
        if self.showToast != showToast { self.showToast = showToast }
        if self.hasTitle != hasTitle { self.hasTitle = hasTitle }
        if self.hasLyrics != hasLyrics { self.hasLyrics = hasLyrics }
        if musicTitle != title { musicTitle = title }
        let tooltip = isPlaying
            ? "♫ \(title) - \(artist) (Click for Music HUD)"
            : "Session & Token Usage"
        if musicTooltip != tooltip { musicTooltip = tooltip }
    }

    func setLyricState(text: String, index: Int, loading: Bool) {
        if currentLyricText != text { currentLyricText = text }
        if currentLyricIndex != index { currentLyricIndex = index }
        if lyricsLoading != loading { lyricsLoading = loading }
    }

    func setTooltip(_ text: String) {
        if tooltipText != text { tooltipText = text }
    }

    func setDetailExpanded(_ expanded: Bool) {
        if isDetailExpanded != expanded { isDetailExpanded = expanded }
    }

    func setChargingBanner(visible: Bool, batteryLevel: Int) {
        withAnimation(.spring(response: 0.38, dampingFraction: 0.84)) {
            if showChargingBanner != visible { showChargingBanner = visible }
            if self.batteryLevel != batteryLevel { self.batteryLevel = batteryLevel }
        }
    }

    func setChargingBannerState(_ state: PowerBannerState) {
        if chargingBannerState != state { chargingBannerState = state }
    }

    func setAlbumPalette(_ palette: AlbumArtPalette, hasArtwork: Bool) {
        if albumPalette != palette || hasAlbumArtwork != hasArtwork {
            albumPalette = palette
            hasAlbumArtwork = hasArtwork
            albumPaletteRevision &+= 1
        }
    }

    func setMiniPeekBanner(_ visible: Bool) {
        withAnimation(.spring(response: 0.38, dampingFraction: 0.84)) {
            if showMiniPeekBanner != visible { showMiniPeekBanner = visible }
        }
    }

    func setVolumeHUDVisible(_ visible: Bool) {
        if isVolumeHUDVisible != visible { isVolumeHUDVisible = visible }
    }

    func setPrayerPrioritized(_ prioritized: Bool) {
        if isPrayerPrioritized != prioritized { isPrayerPrioritized = prioritized }
    }

    func setInQuietHours(_ quiet: Bool) {
        if inQuietHours != quiet { inQuietHours = quiet }
    }
}

/// Shared deadline-aware hover-hold timer for below-notch notification banners.
/// Holds the banner open while hovered, resumes the remaining deadline on early unhover,
/// and dismisses immediately if unhovered after the deadline has elapsed.
@MainActor
public final class BannerHoverHoldTimer {
    public private(set) var isHovered: Bool = false
    public private(set) var deadline: Date?
    private var workItem: DispatchWorkItem?
    private let defaultDuration: TimeInterval

    public init(defaultDuration: TimeInterval = 3.0) {
        self.defaultDuration = defaultDuration
    }

    public func start(duration: TimeInterval? = nil, onDismiss: @escaping () -> Void) {
        let ttl = duration ?? defaultDuration
        isHovered = false
        deadline = Date().addingTimeInterval(ttl)
        schedule(after: ttl, onDismiss: onDismiss)
    }

    public func setHovered(_ hovering: Bool, isVisible: Bool, onDismiss: @escaping () -> Void) {
        guard isVisible else {
            isHovered = false
            return
        }
        if hovering {
            isHovered = true
            workItem?.cancel()
            workItem = nil
        } else if isHovered {
            isHovered = false
            let remaining = deadline?.timeIntervalSinceNow ?? 0
            if remaining > 0 {
                schedule(after: remaining, onDismiss: onDismiss)
            } else {
                cancel()
                onDismiss()
            }
        }
    }

    public func cancel() {
        workItem?.cancel()
        workItem = nil
        isHovered = false
        deadline = nil
    }

    private func schedule(after delay: TimeInterval, onDismiss: @escaping () -> Void) {
        workItem?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self, !self.isHovered else { return }
            self.deadline = nil
            self.workItem = nil
            onDismiss()
        }
        workItem = item
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: item)
    }
}
