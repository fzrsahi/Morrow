import AppKit
import SwiftUI
import MorrowCore
import os.log

enum PanelWindowLayout {
    static func anchoredY(
        screenMaxY: CGFloat,
        panelHeight: CGFloat,
        displayMode: IslandDisplayMode,
        isFullscreenMode: Bool,
        topOffset: Double = DynamicIslandTopOffset.defaultValue
    ) -> CGFloat {
        screenMaxY - panelHeight - CGFloat(IslandDisplayMetrics.floatingYOffset(
            displayMode: displayMode,
            isFullscreenMode: isFullscreenMode,
            customTopOffset: topOffset
        ))
    }
}

enum PanelFullscreenGeometry {
    static func appKitRect(fromCGBounds cgBounds: CGRect, primaryScreenMaxY: CGFloat) -> CGRect {
        let appKitY = primaryScreenMaxY - cgBounds.origin.y - cgBounds.height
        return CGRect(x: cgBounds.origin.x, y: appKitY, width: cgBounds.width, height: cgBounds.height)
    }

    static func isWindowFullscreen(
        cgBounds: CGRect,
        primaryScreenMaxY: CGFloat,
        targetScreenFrame: CGRect
    ) -> Bool {
        let rect = appKitRect(fromCGBounds: cgBounds, primaryScreenMaxY: primaryScreenMaxY)
        let intersection = rect.intersection(targetScreenFrame)
        guard !intersection.isNull, !intersection.isEmpty else { return false }
        return intersection.width >= targetScreenFrame.width - 2
            && intersection.height >= targetScreenFrame.height - 2
    }

    static func candidateWindowBounds(
        from windowInfoList: [[String: Any]],
        frontmostPID: pid_t,
        excludingPID: pid_t = ProcessInfo.processInfo.processIdentifier
    ) -> [CGRect] {
        guard frontmostPID != excludingPID else { return [] }
        var result: [CGRect] = []
        for window in windowInfoList {
            guard let pid = window[kCGWindowOwnerPID as String] as? pid_t,
                  pid == frontmostPID,
                  let layer = window[kCGWindowLayer as String] as? Int, layer == 0,
                  let bounds = window[kCGWindowBounds as String] as? [String: Any],
                  let rect = CGRect(dictionaryRepresentation: bounds as CFDictionary),
                  rect.width > 0,
                  rect.height > 0 else { continue }
            if let alpha = window[kCGWindowAlpha as String] as? NSNumber, alpha.doubleValue <= 0 {
                continue
            }
            result.append(rect)
        }
        return result
    }
}

struct PanelSlotPlan: Equatable {
    let activeDisplayIDs: [String]
    let primaryCoordinatorID: String?
}

enum PanelSlotPlanner {
    static func plan(
        islandEnabled: Bool,
        displays: [DisplayDescriptor],
        configurations: [String: DisplayConfiguration]
    ) -> PanelSlotPlan {
        guard islandEnabled else {
            return PanelSlotPlan(activeDisplayIDs: [], primaryCoordinatorID: nil)
        }
        let activeIDs = displays
            .filter { configurations[$0.id]?.enabled == true }
            .map(\.id)
        return PanelSlotPlan(
            activeDisplayIDs: activeIDs,
            primaryCoordinatorID: activeIDs.first
        )
    }
}

private let log = Logger(subsystem: "com.morrow", category: "Panel")

private class KeyablePanel: NSPanel {
    override var canBecomeKey: Bool { true }

    /// The island deliberately overlaps the menu-bar / notch area. AppKit's
    /// default constrain clamps borderless windows below the menu bar whenever
    /// the system re-places windows (display reconfigure, wake from sleep),
    /// which detaches the panel from the notch (#263).
    override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect {
        frameRect
    }
}

/// Ensures first click on a nonactivatingPanel fires SwiftUI actions
/// instead of being consumed for key-window activation.
/// Also guards against NSHostingView constraint-update re-entrancy crash:
/// during updateConstraints(), SwiftUI may invalidate the view graph and
/// call setNeedsUpdateConstraints again, which AppKit forbids.
private class NotchHostingView<Content: View>: NSHostingView<Content> {
    /// When true, the deferred handler is setting super — don't re-defer.
    private var applyingDeferred = false
    var onScroll: ((NSEvent) -> Bool)?

    override func mouseDown(with event: NSEvent) {
        window?.makeKey()
        super.mouseDown(with: event)
    }

    override func scrollWheel(with event: NSEvent) {
        if SystemVolumeManager.shared.handleScroll(event: event) {
            return
        }
        if let onScroll = onScroll, onScroll(event) {
            return
        }
        super.scrollWheel(with: event)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func layout() {
        PerfCounters.shared?.bump(.hostingLayoutPass)
        super.layout()
    }

    /// Latest requested value awaiting its deferred apply, or nil when nothing
    /// is queued. Coalescing matters: SwiftUI sets these several times per
    /// animation frame, and the mascot drives frames continuously, so one
    /// `DispatchQueue.main.async` per *set* was a permanent stream of closure
    /// allocations and main-queue wakeups on an otherwise idle Mac (#299).
    private var pendingConstraintsValue: Bool?
    private var pendingLayoutValue: Bool?

    /// Always defer `needsUpdateConstraints = true` to the next run-loop turn.
    /// During AppKit's display-cycle (constraint-update or layout phases),
    /// calling setNeedsUpdateConstraints synchronously re-enters
    /// `_postWindowNeedsUpdateConstraints` and throws.  Deferring avoids
    /// that entirely; the one-tick delay is imperceptible.
    override var needsUpdateConstraints: Bool {
        get { super.needsUpdateConstraints }
        set {
            if applyingDeferred {
                super.needsUpdateConstraints = newValue
                return
            }
            // Nothing queued and already in the requested state — no-op.
            if pendingConstraintsValue == nil, super.needsUpdateConstraints == newValue { return }
            let alreadyScheduled = pendingConstraintsValue != nil
            pendingConstraintsValue = newValue
            guard !alreadyScheduled else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self, let value = self.pendingConstraintsValue else { return }
                self.pendingConstraintsValue = nil
                self.applySuperNeedsUpdateConstraints(value)
            }
        }
    }

    private func applySuperNeedsUpdateConstraints(_ value: Bool) {
        applyingDeferred = true
        super.needsUpdateConstraints = value
        applyingDeferred = false
    }

    override var needsLayout: Bool {
        get { super.needsLayout }
        set {
            if applyingDeferred {
                super.needsLayout = newValue
                return
            }
            if pendingLayoutValue == nil, super.needsLayout == newValue { return }
            let alreadyScheduled = pendingLayoutValue != nil
            pendingLayoutValue = newValue
            guard !alreadyScheduled else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self, let value = self.pendingLayoutValue else { return }
                self.pendingLayoutValue = nil
                self.applySuperNeedsLayout(value)
            }
        }
    }

    private func applySuperNeedsLayout(_ value: Bool) {
        applyingDeferred = true
        super.needsLayout = value
        applyingDeferred = false
    }
}

struct PanelScreenHopFrames {
    let outgoing: NSRect
    let incoming: NSRect
}

struct PanelScreenHopMotion {
    let outgoingOffset: CGFloat
    let incomingOffset: CGFloat
    let fadeOutDuration: TimeInterval
    let incomingPauseDuration: TimeInterval
    let fadeInDuration: TimeInterval
}

enum PanelHeightMetrics {
    /// Window height the panel asks for before clamping to the screen: room
    /// for `maxVisibleSessions` cards plus the notch bar. Shared with the
    /// completion card, whose reply area has to fit inside the same window.
    static func desiredHeight(maxVisibleSessions: Int) -> CGFloat {
        let sessions = CGFloat(max(2, maxVisibleSessions))
        return max(300, sessions * 90 + 60)
    }
}

@MainActor
class PanelWindowController: NSObject, NSWindowDelegate {
    private enum ScreenHopMetrics {
        static let outgoingOffset: CGFloat = 18
        static let incomingOffset: CGFloat = 30
        static let fadeOutDuration: TimeInterval = 0.14
        static let incomingPauseDuration: TimeInterval = 0.06
        static let fadeInDuration: TimeInterval = 0.34
    }

    nonisolated static func screenHopMotion() -> PanelScreenHopMotion {
        PanelScreenHopMotion(
            outgoingOffset: ScreenHopMetrics.outgoingOffset,
            incomingOffset: ScreenHopMetrics.incomingOffset,
            fadeOutDuration: ScreenHopMetrics.fadeOutDuration,
            incomingPauseDuration: ScreenHopMetrics.incomingPauseDuration,
            fadeInDuration: ScreenHopMetrics.fadeInDuration
        )
    }

    private final class PanelSlot {
        let displayID: String
        let panel: KeyablePanel
        var hostingView: NotchHostingView<NotchPanelView>
        var screenSignature: String
        var configuration: DisplayConfiguration
        var isPrimaryCoordinator: Bool
        var isFullscreen: Bool = false

        init(
            displayID: String,
            panel: KeyablePanel,
            hostingView: NotchHostingView<NotchPanelView>,
            screenSignature: String,
            configuration: DisplayConfiguration,
            isPrimaryCoordinator: Bool
        ) {
            self.displayID = displayID
            self.panel = panel
            self.hostingView = hostingView
            self.screenSignature = screenSignature
            self.configuration = configuration
            self.isPrimaryCoordinator = isPrimaryCoordinator
        }
    }

    private struct SettingsSnapshot: Equatable {
        let islandEnabled: Bool
        let displayConfigsRaw: String
        let displayChoice: String
        let notchHeightMode: String
        let customNotchHeight: Double
        let islandDisplayMode: IslandDisplayMode
        let dynamicIslandTopOffset: Double
        let maxVisibleSessions: Int
        let allowHorizontalDrag: Bool
        let avoidMenuBarIcons: Bool
        let panelHorizontalOffset: Double
        let hideInFullscreen: Bool
        let hideWhenNoSession: Bool

        @MainActor
        static func current() -> SettingsSnapshot {
            let defaults = UserDefaults.standard
            let settings = SettingsManager.shared
            let rawConfigs: String
            if let str = defaults.string(forKey: SettingsKey.displayConfigurations) {
                rawConfigs = str
            } else if let data = defaults.data(forKey: SettingsKey.displayConfigurations),
                      let str = String(data: data, encoding: .utf8) {
                rawConfigs = str
            } else {
                rawConfigs = ""
            }
            return SettingsSnapshot(
                islandEnabled: settings.islandEnabled,
                displayConfigsRaw: rawConfigs,
                displayChoice: settings.displayChoice,
                notchHeightMode: settings.notchHeightMode.rawValue,
                customNotchHeight: settings.customNotchHeight,
                islandDisplayMode: settings.islandDisplayMode,
                dynamicIslandTopOffset: settings.dynamicIslandTopOffset,
                maxVisibleSessions: defaults.integer(forKey: SettingsKey.maxVisibleSessions),
                allowHorizontalDrag: settings.allowHorizontalDrag,
                avoidMenuBarIcons: settings.avoidMenuBarIcons,
                panelHorizontalOffset: settings.panelHorizontalOffset,
                hideInFullscreen: settings.hideInFullscreen,
                hideWhenNoSession: settings.hideWhenNoSession
            )
        }
    }

    struct ScreenEntry {
        let descriptor: DisplayDescriptor
        let screen: NSScreen
        var frameOverride: CGRect? = nil
        var frame: CGRect { frameOverride ?? screen.frame }
    }

    var screenEntriesProvider: () -> [ScreenEntry] = {
        ScreenDetector.screenDescriptors().map {
            ScreenEntry(descriptor: $0.descriptor, screen: $0.screen)
        }
    }

    var activeSlotFramesForTesting: [String: NSRect] {
        slots.mapValues { $0.panel.frame }
    }

    var activeSlotModesForTesting: [String: IslandDisplayMode] {
        slots.mapValues { $0.configuration.displayMode }
    }

    func syncPanelsForTesting() {
        syncPanels(forceRebuild: false)
    }

    func tearDownAllSlotsForTesting() {
        for id in Array(slots.keys) {
            tearDownSlot(displayID: id)
        }
    }

    private var slots: [String: PanelSlot] = [:]
    private var hoveredDisplayIDs: Set<String> = []
    private let appState: AppState

    nonisolated static func screenHopFrames(
        oldFrame: NSRect,
        newFrame: NSRect
    ) -> PanelScreenHopFrames {
        let motion = screenHopMotion()
        return PanelScreenHopFrames(
            outgoing: oldFrame.offsetBy(dx: 0, dy: motion.outgoingOffset),
            incoming: newFrame.offsetBy(dx: 0, dy: motion.incomingOffset)
        )
    }

    private func panelSize(for entry: ScreenEntry) -> NSSize {
        let desiredH = PanelHeightMetrics.desiredHeight(
            maxVisibleSessions: UserDefaults.standard.integer(forKey: SettingsKey.maxVisibleSessions)
        )
        // Clamp to the screen's usable height. A borderless, non-opaque panel with a
        // very tall backing store (e.g. maxVisibleSessions=99 → 8970pt, ~18k px tall on
        // a Retina display) has its compositing dropped by the macOS 26 WindowServer
        // once the surface goes idle — the card renders blank until a mouse event forces
        // a redraw, which looked like "invisible after ~2s, reappears on hover" (#304).
        // The content is top-anchored and the session list scrolls internally, so the
        // window never needs to be taller than the visible screen.
        let visibleHeight = entry.frameOverride?.height ?? entry.screen.visibleFrame.height
        let maxH = min(desiredH, visibleHeight)
        let screenW = entry.frame.width
        let width = min(760, screenW - 40)
        return NSSize(width: width, height: maxH)
    }

    private var fullscreenPoller: Timer?
    private var isSessionObservationArmed = false
    private var hasStarted = false
    private var settingsObservers: [NSObjectProtocol] = []
    private var defaultCenterObservers: [NSObjectProtocol] = []
    private var workspaceObservers: [NSObjectProtocol] = []
    private var globalClickMonitor: Any?
    private var dragTargetDisplayID: String?
    private var dragStartMouseX: CGFloat?
    private var dragStartPanelX: CGFloat?
    private var isDraggingPanel = false
    private var localDragMonitor: Any?
    private var lastSettingsSnapshot: SettingsSnapshot?

    init(appState: AppState) {
        self.appState = appState
        super.init()
    }

    /// Watch `sessions` / `surface` for changes that affect panel visibility.
    ///
    /// `withObservationTracking` is one-shot, so it has to be re-armed. Doing
    /// that from a 500 ms polling loop meant a permanent timer wakeup on a Mac
    /// where nothing was happening (#299) — and it bought nothing, because the
    /// loop never *detected* anything: `onChange` already fires the instant a
    /// tracked property is written. Re-arm from `onChange` instead, so an idle
    /// app schedules no work at all.
    ///
    /// Re-arming happens in the same main-actor turn as `updateVisibility()`,
    /// with no await between them, so no mutation can slip through the window
    /// where tracking is momentarily disarmed.
    private func armSessionObservation() {
        guard !isSessionObservationArmed else { return }
        isSessionObservationArmed = true
        withObservationTracking {
            _ = appState.sessions
            _ = appState.surface
            _ = IslandSignals.shared.isPlaying
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.isSessionObservationArmed = false
                // A session event is the user's cue to look again: it revives a
                // waiting island that had gone quiet.
                MascotAnimationGate.shared.noteInteraction()
                self.updateVisibility()
                self.armSessionObservation()
            }
        }
    }

    func showPanel() {
        syncPanels(forceRebuild: true)

        guard !hasStarted else { return }
        hasStarted = true

        // A follow-up for the card under the pointer would remind someone who
        // is already reading it. The island's own hover says whether the
        // pointer is on the card (`pointerOverIsland`); the window frame —
        // mostly transparent, and as large as the card ever gets — only
        // backs that up against a hover flag a rebuilt view left behind.
        appState.followUps.isPointerOverPanel = { [weak self] in
            guard let self else { return false }
            let mouse = NSEvent.mouseLocation
            return self.slots.values.contains { $0.panel.isVisible && $0.panel.frame.contains(mouse) }
        }

        setupHorizontalDragMonitor()

        // Screen change observer
        let screenObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.syncPanels(forceRebuild: true)
                // macOS may not have finished updating NSScreen.screens when the notification fires.
                // Rebuild again after a short delay to pick up the final screen configuration.
                try? await Task.sleep(nanoseconds: 500_000_000)
                self?.syncPanels(forceRebuild: true)
            }
        }
        defaultCenterObservers.append(screenObserver)

        // Active space change — check fullscreen per display
        let spaceObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.activeSpaceDidChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                self.syncPanels()
            }
        }
        workspaceObservers.append(spaceObserver)

        // Frontmost app change — refresh per-display fullscreen state and visibility
        let appObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                self.refreshFullscreenStates(updateViews: true)
                self.updateAllPositions()
                self.updateVisibility()
                self.syncFullscreenPoller()
            }
        }
        workspaceObservers.append(appObserver)

        // Observe session changes via @Observable tracking
        armSessionObservation()

        // Observe settings changes (multi-display config, panel height)
        observeSettingsChanges()

        // Global click monitor: close panel when clicking outside all active panels
        globalClickMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] _ in
            Task { @MainActor in
                guard let self, self.appState.surface.isExpanded else { return }
                // Don't close during approval/question
                switch self.appState.surface {
                case .approvalCard, .questionCard: return
                default: break
                }
                let clickLocation = NSEvent.mouseLocation
                if self.slots.values.contains(where: { $0.panel.isVisible && $0.panel.frame.contains(clickLocation) }) {
                    return
                }
                withAnimation(NotchAnimation.close) {
                    self.appState.surface = .collapsed
                    self.appState.cancelCompletionQueue()
                }
            }
        }
    }

    private func makeNotchPanelView(
        for entry: ScreenEntry,
        displayID: String,
        configuration: DisplayConfiguration,
        isFullscreen: Bool,
        isPrimaryCoordinator: Bool
    ) -> NotchPanelView {
        let hasNotch = entry.frameOverride != nil ? entry.descriptor.hasNotch : ScreenDetector.screenHasNotch(entry.screen)
        let notchHeight = entry.frameOverride != nil ? (entry.descriptor.hasNotch ? 32 : 25) : ScreenDetector.topBarHeight(for: entry.screen)
        let notchW = entry.frameOverride != nil ? (entry.descriptor.hasNotch ? 185 : min(max(entry.frame.width * 0.14, 160), 240)) : ScreenDetector.notchWidth(for: entry.screen)

        return NotchPanelView(
            appState: appState,
            hasNotch: hasNotch,
            notchHeight: notchHeight,
            notchW: notchW,
            screenWidth: entry.frame.width,
            displayModeOverride: configuration.displayMode,
            isFullscreenOverride: isFullscreen,
            isPrimaryCoordinator: isPrimaryCoordinator,
            onHoverChanged: { [weak self] hovering in
                self?.updateHoverState(displayID: displayID, hovering: hovering)
            }
        )
    }

    private func makeHostingView(
        for entry: ScreenEntry,
        displayID: String,
        configuration: DisplayConfiguration,
        isFullscreen: Bool,
        isPrimaryCoordinator: Bool
    ) -> NotchHostingView<NotchPanelView> {
        let rootView = makeNotchPanelView(
            for: entry,
            displayID: displayID,
            configuration: configuration,
            isFullscreen: isFullscreen,
            isPrimaryCoordinator: isPrimaryCoordinator
        )
        let contentView = NotchHostingView(rootView: rootView)
        contentView.onScroll = { [weak self] event in
            guard let self else { return false }
            return NotchNavigationManager.shared.handleScroll(event: event, appState: self.appState)
        }
        contentView.sizingOptions = []
        contentView.translatesAutoresizingMaskIntoConstraints = true
        return contentView
    }

    private func makePanel(for entry: ScreenEntry, contentView: NotchHostingView<NotchPanelView>) -> KeyablePanel {
        let size = panelSize(for: entry)
        let panel = KeyablePanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isFloatingPanel = true
        panel.acceptsMouseMovedEvents = true
        panel.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.mainMenuWindow)) + 2)
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.isMovableByWindowBackground = false
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.sharingType = .readOnly
        panel.isReleasedWhenClosed = false
        panel.contentView = contentView
        panel.delegate = self
        return panel
    }

    private func slotSignature(for screen: NSScreen) -> String {
        let base = ScreenDetector.signature(for: screen)
        let hasNotch = ScreenDetector.screenHasNotch(screen) ? 1 : 0
        let height = Int(ScreenDetector.topBarHeight(for: screen).rounded())
        let notchW = Int(ScreenDetector.notchWidth(for: screen).rounded())
        return "\(base):n\(hasNotch):h\(height):w\(notchW)"
    }

    private func updateHoverState(displayID: String, hovering: Bool) {
        if hovering {
            hoveredDisplayIDs.insert(displayID)
        } else {
            hoveredDisplayIDs.remove(displayID)
        }
        appState.followUps.pointerOverIsland = !hoveredDisplayIDs.isEmpty
    }

    private func tearDownSlot(displayID: String) {
        guard let slot = slots.removeValue(forKey: displayID) else { return }
        hoveredDisplayIDs.remove(displayID)
        appState.followUps.pointerOverIsland = !hoveredDisplayIDs.isEmpty
        if dragTargetDisplayID == displayID {
            dragTargetDisplayID = nil
            dragStartMouseX = nil
            dragStartPanelX = nil
            isDraggingPanel = false
        }
        slot.hostingView.onScroll = nil
        slot.panel.contentView = nil
        slot.panel.delegate = nil
        slot.panel.orderOut(nil)
        slot.panel.close()
    }

    private func syncPanels(forceRebuild: Bool = false) {
        let entries = screenEntriesProvider()
        let displays = entries.map(\.descriptor)
        let entriesByID = Dictionary(uniqueKeysWithValues: entries.map { ($0.descriptor.id, $0) })

        let islandEnabled = SettingsManager.shared.islandEnabled
        let configs = SettingsManager.shared.displayConfigurations(for: displays)
        let plan = PanelSlotPlanner.plan(
            islandEnabled: islandEnabled,
            displays: displays,
            configurations: configs
        )
        let activeSet = Set(plan.activeDisplayIDs)

        for existingID in Array(slots.keys) where !activeSet.contains(existingID) {
            tearDownSlot(displayID: existingID)
        }

        for displayID in plan.activeDisplayIDs {
            guard let entry = entriesByID[displayID],
                  let config = configs[displayID] else { continue }

            let isCoordinator = (displayID == plan.primaryCoordinatorID)
            let signature = entry.frameOverride != nil
                ? "\(entry.descriptor.id):\(Int(entry.frame.minX))x\(Int(entry.frame.minY)):\(Int(entry.frame.width))x\(Int(entry.frame.height))"
                : slotSignature(for: entry.screen)

            if let slot = slots[displayID] {
                let needsViewRefresh = forceRebuild
                    || slot.screenSignature != signature
                    || slot.configuration.displayMode != config.displayMode
                    || slot.isPrimaryCoordinator != isCoordinator

                slot.configuration = config
                slot.isPrimaryCoordinator = isCoordinator
                slot.screenSignature = signature

                if needsViewRefresh {
                    slot.hostingView.rootView = makeNotchPanelView(
                        for: entry,
                        displayID: displayID,
                        configuration: config,
                        isFullscreen: slot.isFullscreen,
                        isPrimaryCoordinator: isCoordinator
                    )
                }
            } else {
                let hostingView = makeHostingView(
                    for: entry,
                    displayID: displayID,
                    configuration: config,
                    isFullscreen: false,
                    isPrimaryCoordinator: isCoordinator
                )
                let panel = makePanel(for: entry, contentView: hostingView)
                slots[displayID] = PanelSlot(
                    displayID: displayID,
                    panel: panel,
                    hostingView: hostingView,
                    screenSignature: signature,
                    configuration: config,
                    isPrimaryCoordinator: isCoordinator
                )
            }
        }

        refreshFullscreenStates(updateViews: true, entriesByID: entriesByID)
        updateAllPositions(entriesByID: entriesByID)
        updateVisibility()
        syncFullscreenPoller()
    }

    private func observeSettingsChanges() {
        lastSettingsSnapshot = SettingsSnapshot.current()
        let observer = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                let next = SettingsSnapshot.current()
                guard let prev = self.lastSettingsSnapshot else {
                    self.lastSettingsSnapshot = next
                    self.syncPanels(forceRebuild: true)
                    return
                }
                guard next != prev else { return }
                self.lastSettingsSnapshot = next

                let heightChanged = next.notchHeightMode != prev.notchHeightMode
                    || abs(next.customNotchHeight - prev.customNotchHeight) > 0.001
                let slotStructureChanged = next.islandEnabled != prev.islandEnabled
                    || next.displayConfigsRaw != prev.displayConfigsRaw
                    || next.displayChoice != prev.displayChoice
                    || next.islandDisplayMode != prev.islandDisplayMode
                    || abs(next.dynamicIslandTopOffset - prev.dynamicIslandTopOffset) > 0.001
                    || heightChanged

                if slotStructureChanged {
                    self.syncPanels(forceRebuild: heightChanged)
                    return
                }

                if next.maxVisibleSessions != prev.maxVisibleSessions
                    || next.allowHorizontalDrag != prev.allowHorizontalDrag
                    || next.avoidMenuBarIcons != prev.avoidMenuBarIcons
                    || abs(next.panelHorizontalOffset - prev.panelHorizontalOffset) > 0.001 {
                    self.updateAllPositions()
                }

                if next.hideInFullscreen != prev.hideInFullscreen
                    || next.hideWhenNoSession != prev.hideWhenNoSession {
                    self.updateVisibility()
                }
            }
        }
        settingsObservers.append(observer)
    }

    private func currentScreenEntriesByID() -> [String: ScreenEntry] {
        Dictionary(uniqueKeysWithValues: screenEntriesProvider().map { ($0.descriptor.id, $0) })
    }

    private func updateAllPositions(entriesByID: [String: ScreenEntry]? = nil) {
        let lookup = entriesByID ?? currentScreenEntriesByID()
        for (displayID, slot) in slots {
            guard let entry = lookup[displayID] else { continue }
            let target = panelFrame(for: entry, configuration: slot.configuration, isFullscreen: slot.isFullscreen)
            guard slot.panel.frame != target else { continue }
            slot.panel.setFrame(target, display: true)
        }
    }

    private func panelFrame(
        for entry: ScreenEntry,
        configuration: DisplayConfiguration,
        isFullscreen: Bool
    ) -> NSRect {
        let size = panelSize(for: entry)
        let screenFrame = entry.frame
        let centeredX = centeredX(for: size, frame: screenFrame)
        let dragOffset = SettingsManager.shared.allowHorizontalDrag
            ? CGFloat(SettingsManager.shared.panelHorizontalOffset)
            : 0
        var desiredX = centeredX + dragOffset

        // #219: on external screens the simulated notch can land on third-party
        // status items (Bartender & co.) — slide it into the nearest clear gap.
        // A user-dragged offset is an explicit choice and always wins.
        if SettingsManager.shared.avoidMenuBarIcons,
           dragOffset == 0,
           entry.frameOverride == nil,
           !ScreenDetector.screenHasNotch(entry.screen) {
            let occupied = MenuBarIconAvoidance.occupiedMenuBarRanges(on: entry.screen)
            desiredX = MenuBarIconAvoidance.resolvedX(
                preferredX: desiredX,
                panelWidth: size.width,
                occupied: occupied,
                screenMinX: screenFrame.minX,
                screenMaxX: screenFrame.maxX,
                maxShift: screenFrame.width * MenuBarIconAvoidance.maxShiftFraction
            )
        }

        let x = clampedX(desiredX, panelWidth: size.width, frame: screenFrame)
        let y = anchoredY(for: size.height, screenMaxY: screenFrame.maxY, configuration: configuration, isFullscreen: isFullscreen)
        return NSRect(x: x, y: y, width: size.width, height: size.height)
    }

    private func anchoredY(
        for panelHeight: CGFloat,
        screenMaxY: CGFloat,
        configuration: DisplayConfiguration,
        isFullscreen: Bool
    ) -> CGFloat {
        PanelWindowLayout.anchoredY(
            screenMaxY: screenMaxY,
            panelHeight: panelHeight,
            displayMode: configuration.displayMode,
            isFullscreenMode: isFullscreen,
            topOffset: configuration.dynamicIslandTopOffset
        )
    }

    private func centeredX(for size: NSSize, frame: CGRect) -> CGFloat {
        frame.midX - size.width / 2
    }

    private func clampedX(_ desiredX: CGFloat, panelWidth: CGFloat, frame: CGRect) -> CGFloat {
        min(max(desiredX, frame.minX), frame.maxX - panelWidth)
    }

    private func setupHorizontalDragMonitor() {
        let dragThreshold: CGFloat = 5

        localDragMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp]) { [weak self] event in
            guard let self,
                  SettingsManager.shared.allowHorizontalDrag,
                  self.appState.expandedTab != .game,
                  !self.appState.isGameInteractionLocked else { return event }

            switch event.type {
            case .leftMouseDown:
                if let matchedSlot = self.slots.values.first(where: { $0.panel === event.window }) {
                    self.dragTargetDisplayID = matchedSlot.displayID
                    self.dragStartMouseX = NSEvent.mouseLocation.x
                    self.dragStartPanelX = matchedSlot.panel.frame.origin.x
                    self.isDraggingPanel = false
                }
            case .leftMouseDragged:
                if let targetID = self.dragTargetDisplayID,
                   let slot = self.slots[targetID],
                   let entry = self.currentScreenEntriesByID()[targetID],
                   let startMouseX = self.dragStartMouseX,
                   let startPanelX = self.dragStartPanelX {
                    let deltaX = NSEvent.mouseLocation.x - startMouseX
                    // Only start moving after exceeding threshold
                    if !self.isDraggingPanel {
                        guard abs(deltaX) > dragThreshold else { return event }
                        self.isDraggingPanel = true
                    }
                    let size = slot.panel.frame.size
                    let newX = self.clampedX(startPanelX + deltaX, panelWidth: size.width, frame: entry.frame)
                    let fixedY = self.anchoredY(
                        for: size.height,
                        screenMaxY: entry.frame.maxY,
                        configuration: slot.configuration,
                        isFullscreen: slot.isFullscreen
                    )
                    slot.panel.setFrameOrigin(NSPoint(x: newX, y: fixedY))
                }
            case .leftMouseUp:
                if self.isDraggingPanel,
                   let targetID = self.dragTargetDisplayID,
                   let slot = self.slots[targetID],
                   let entry = self.currentScreenEntriesByID()[targetID] {
                    let size = slot.panel.frame.size
                    let offset = slot.panel.frame.origin.x - self.centeredX(for: size, frame: entry.frame)
                    SettingsManager.shared.panelHorizontalOffset = Double(offset)
                }
                self.dragTargetDisplayID = nil
                self.dragStartMouseX = nil
                self.dragStartPanelX = nil
                self.isDraggingPanel = false
            default:
                break
            }
            return event
        }
    }

    @discardableResult
    private func refreshFullscreenStates(
        updateViews: Bool,
        entriesByID: [String: ScreenEntry]? = nil
    ) -> Bool {
        let lookup = entriesByID ?? currentScreenEntriesByID()
        let windowList = fullscreenCandidateWindowBounds()
        let primaryMaxY = (NSScreen.screens.first(where: { $0.frame.origin == .zero }) ?? NSScreen.screens.first)?.frame.maxY ?? 0

        var anyChanged = false
        for (displayID, slot) in slots {
            guard let entry = lookup[displayID] else { continue }
            let nextFullscreen = windowList.contains { cgBounds in
                PanelFullscreenGeometry.isWindowFullscreen(
                    cgBounds: cgBounds,
                    primaryScreenMaxY: primaryMaxY,
                    targetScreenFrame: entry.frame
                )
            }
            if slot.isFullscreen != nextFullscreen {
                slot.isFullscreen = nextFullscreen
                anyChanged = true
                if updateViews {
                    slot.hostingView.rootView = makeNotchPanelView(
                        for: entry,
                        displayID: displayID,
                        configuration: slot.configuration,
                        isFullscreen: nextFullscreen,
                        isPrimaryCoordinator: slot.isPrimaryCoordinator
                    )
                }
            }
        }
        appState.isFullscreenMode = slots.values.contains(where: \.isFullscreen)
        return anyChanged
    }

    private func syncFullscreenPoller() {
        if slots.values.contains(where: \.isFullscreen) {
            if fullscreenPoller == nil {
                startFullscreenExitPoller()
            }
        } else {
            fullscreenPoller?.invalidate()
            fullscreenPoller = nil
        }
    }

    /// Poll every 1.5s while any display is in fullscreen; stop when all exit fullscreen
    private func startFullscreenExitPoller() {
        fullscreenPoller?.invalidate()
        let timer = Timer.scheduledTimer(withTimeInterval: 1.5, repeats: true) { [weak self] timer in
            Task { @MainActor in
                guard let self else { timer.invalidate(); return }
                let changed = self.refreshFullscreenStates(updateViews: true)
                if changed {
                    self.updateAllPositions()
                    self.updateVisibility()
                }
                if !self.slots.values.contains(where: \.isFullscreen) {
                    timer.invalidate()
                    self.fullscreenPoller = nil
                }
            }
        }
        // A fullscreen exit is noticed a moment later; nothing depends on 1.5 s exactly.
        timer.tolerance = 0.15
        fullscreenPoller = timer
    }

    /// Update panel visibility across all active display slots based on settings
    private func updateVisibility() {
        let settings = SettingsManager.shared
        let hasActiveBanner = SystemMonitorManager.shared.showChargingBanner || PrayerTimeManager.shared.showMiniPeekBanner
        if !settings.islandEnabled || (settings.hideWhenNoSession && appState.activeSessionCount == 0 && !hasActiveBanner) {
            for slot in slots.values {
                slot.panel.orderOut(nil)
            }
            MascotAnimationGate.shared.setPanelVisible(false)
            return
        }

        var anyVisible = false
        for slot in slots.values {
            if settings.hideInFullscreen && slot.isFullscreen {
                slot.panel.orderOut(nil)
            } else {
                if !slot.panel.isVisible {
                    slot.panel.orderFrontRegardless()
                }
                anyVisible = true
            }
        }

        MascotAnimationGate.shared.setPanelVisible(anyVisible)
        if anyVisible {
            MascotAnimationGate.shared.setIdleSettled(isIslandSettled())
            MascotAnimationGate.shared.setMusicPlaying(NowPlayingManager.shared.isPlaying)
            // An expanded island is being looked at, so Now Playing may probe more often.
            NowPlayingManager.shared.isIslandExpanded = appState.surface.isExpanded
            MascotAnimationGate.shared.setWaitingForUser(isWaitingForUser())
        }
    }

    /// Collapsed island with nothing running — the state a Mac spends most of
    /// its day in, and the one where a live mascot timeline costs real CPU for
    /// motion nobody is looking at (#299). Any expanded surface, any session
    /// that is not idle, or a playing track keeps the island animating.
    private func isIslandSettled() -> Bool {
        guard !appState.surface.isExpanded else { return false }
        guard !NowPlayingManager.shared.isPlaying else { return false }
        return appState.sessions.values.allSatisfy { $0.status == .idle }
    }

    /// A session parked on an approval or a question is waiting for the user.
    private func isWaitingForUser() -> Bool {
        appState.sessions.values.contains {
            $0.status == .waitingApproval || $0.status == .waitingQuestion
        }
    }

    private func fullscreenCandidateWindowBounds() -> [CGRect] {
        guard let frontApp = NSWorkspace.shared.frontmostApplication,
              frontApp.processIdentifier != ProcessInfo.processInfo.processIdentifier,
              let windowList = CGWindowListCopyWindowInfo(
                  [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID
              ) as? [[String: Any]] else {
            return []
        }

        return PanelFullscreenGeometry.candidateWindowBounds(
            from: windowList,
            frontmostPID: frontApp.processIdentifier,
            excludingPID: ProcessInfo.processInfo.processIdentifier
        )
    }

    /// Whether any island panel currently holds keyboard focus. It only becomes key
    /// when the user clicks into it (NotchHostingView.mouseDown).
    var isPanelKeyWindow: Bool {
        slots.values.contains { $0.panel.isKeyWindow }
    }

    /// Fast check: is the terminal running the active session the foreground app?
    /// Main-thread safe — no AppleScript or subprocess calls.
    func isActiveTerminalForeground() -> Bool {
        guard let sessionId = appState.activeSessionId,
              let session = appState.sessions[sessionId],
              session.termApp != nil else { return false }
        return TerminalVisibilityDetector.isTerminalFrontmostForSession(session)
    }

    func windowDidMove(_ notification: Notification) {
        // Horizontal drag (setupHorizontalDragMonitor) only ever changes x.
        // A y drifted off the top edge therefore means the system re-placed the panel —
        // e.g. a display reconfigure clamping it below the menu bar (#263) — so snap it back.
        guard !isDraggingPanel,
              let movedWindow = notification.object as? NSWindow,
              let slot = slots.values.first(where: { $0.panel === movedWindow }),
              let entry = currentScreenEntriesByID()[slot.displayID] else { return }
        let expectedY = anchoredY(
            for: slot.panel.frame.height,
            screenMaxY: entry.frame.maxY,
            configuration: slot.configuration,
            isFullscreen: slot.isFullscreen
        )
        guard abs(slot.panel.frame.origin.y - expectedY) > 0.5 else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.isDraggingPanel else { return }
            self.updateAllPositions()
        }
    }

    deinit {
        fullscreenPoller?.invalidate()
        for observer in settingsObservers {
            NotificationCenter.default.removeObserver(observer)
        }
        for observer in defaultCenterObservers {
            NotificationCenter.default.removeObserver(observer)
        }
        for observer in workspaceObservers {
            NSWorkspace.shared.notificationCenter.removeObserver(observer)
        }
        if let monitor = globalClickMonitor {
            NSEvent.removeMonitor(monitor)
        }
        if let monitor = localDragMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }
}
