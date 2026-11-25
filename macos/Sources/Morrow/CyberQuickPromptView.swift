import SwiftUI
import AppKit
import MorrowCore

struct CyberQuickPromptView: View {
    var appState: AppState
    @State private var promptText: String = ""
    @State private var isSubmitting: Bool = false
    @State private var statusFeedback: String? = nil
    @FocusState private var isFieldFocused: Bool
    @AppStorage(SettingsKey.musicHUDProgressColor) private var musicHUDProgressColor = SettingsDefaults.musicHUDProgressColor
    @AppStorage(SettingsKey.syncThemeWithAlbumArt) private var syncThemeWithAlbumArt = SettingsDefaults.syncThemeWithAlbumArt
    @AppStorage(SettingsKey.islandUITheme) private var islandUITheme = SettingsDefaults.islandUITheme
    private let signals = IslandSignals.shared

    private var isPixel: Bool {
        islandUITheme == IslandUITheme.pixel.rawValue
    }

    private var accentColor: Color {
        _ = signals.isPlaying
        _ = signals.albumPaletteRevision
        _ = syncThemeWithAlbumArt
        return AppColorTheme.readableOnDark(
            AppColorTheme.accent(for: musicHUDProgressColor, isModern: !isPixel)
        )
    }

    init(appState: AppState) {
        self.appState = appState
    }

    private var activeSession: SessionSnapshot? {
        let sid = appState.activeSessionId ?? appState.sessions.keys.sorted().first
        guard let sid else { return nil }
        return appState.sessions[sid]
    }

    private var targetAgentName: String {
        if let s = activeSession {
            let src = s.mascotSource.isEmpty ? s.source : s.mascotSource
            return src.uppercased()
        }
        return "AI AGENT"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            // Target telemetry badge + Close button
            HStack(spacing: 6) {
                HStack(spacing: 3) {
                    Circle()
                        .fill(accentColor)
                        .frame(width: 4.5, height: 4.5)
                    Text("DISPATCH TO: \(targetAgentName)")
                        .font(.system(size: 7.5, weight: .bold, design: isPixel ? .monospaced : .rounded))
                        .foregroundStyle(accentColor)
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(
                    Group {
                        if isPixel {
                            RoundedRectangle(cornerRadius: 3).fill(accentColor.opacity(0.12))
                        } else {
                            Capsule().fill(accentColor.opacity(0.14))
                        }
                    }
                )

                Spacer()

                Button(action: {
                    withAnimation(NotchAnimation.close) {
                        appState.surface = .collapsed
                    }
                }) {
                    Image(systemName: "xmark")
                        .font(.system(size: 7, weight: .bold))
                        .foregroundStyle(Color.white.opacity(0.5))
                        .frame(width: 16, height: 16)
                        .background(Circle().fill(Color(white: 0.12)))
                }
                .buttonStyle(.plain)
            }

            // Glowing Prompt Input Box
            HStack(spacing: 6) {
                Image(systemName: "terminal.fill")
                    .font(.system(size: 9))
                    .foregroundStyle(accentColor)

                TextField("Type instruction or prompt for active agent...", text: $promptText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 10.5, weight: .medium, design: isPixel ? .monospaced : .rounded))
                    .foregroundStyle(Color.white)
                    .focused($isFieldFocused)
                    .onSubmit {
                        submitPrompt()
                    }

                Button(action: { submitPrompt() }) {
                    HStack(spacing: 3) {
                        Image(systemName: "arrow.up.circle.fill")
                            .font(.system(size: 9))
                        Text("RUN")
                            .font(.system(size: 7.5, weight: .bold, design: isPixel ? .monospaced : .rounded))
                    }
                    .foregroundStyle(AppColorTheme.contrastingForeground(on: accentColor))
                    .padding(.horizontal, isPixel ? 7 : 9)
                    .padding(.vertical, 3.5)
                    .background(
                        Group {
                            if isPixel {
                                RoundedRectangle(cornerRadius: 3.5).fill(accentColor)
                            } else {
                                Capsule().fill(accentColor)
                            }
                        }
                    )
                }
                .buttonStyle(.plain)
                .disabled(promptText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSubmitting)
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 6)
            .background(
                Group {
                    if isPixel {
                        RoundedRectangle(cornerRadius: 4.5)
                            .fill(Pixel16Palette.slot)
                            .pixel16Bevel(raised: false, fillColor: Pixel16Palette.slot)
                    } else {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(Color.black.opacity(0.45))
                            .overlay(
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .strokeBorder(Color.white.opacity(0.10), lineWidth: 0.8)
                            )
                    }
                }
            )

            // Bottom Hints & Status Feedback
            HStack {
                if let feedback = statusFeedback {
                    Text(feedback)
                        .font(.system(size: 7.5, weight: .bold, design: isPixel ? .monospaced : .rounded))
                        .foregroundStyle(accentColor)
                } else {
                    Text("Press Enter to send · Esc to cancel")
                        .font(.system(size: 7, design: isPixel ? .monospaced : .rounded))
                        .foregroundStyle(Color.white.opacity(0.45))
                }
                Spacer()
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            Group {
                if isPixel {
                    RoundedRectangle(cornerRadius: 5)
                        .fill(Pixel16Palette.chassis)
                        .pixel16Bevel(raised: false, fillColor: Pixel16Palette.chassis)
                } else {
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .fill(Color.white.opacity(0.05))
                        .overlay(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .strokeBorder(Color.white.opacity(0.10), lineWidth: 0.8)
                        )
                }
            }
        )
        .padding(.horizontal, isPixel ? 6 : 12)
        .padding(.bottom, isPixel ? 6 : 10)
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                isFieldFocused = true
            }
        }
    }

    private func submitPrompt() {
        let text = promptText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        isSubmitting = true
        NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .default)

        // 1. If active session exists, activate terminal and paste/send prompt
        if let sid = appState.activeSessionId ?? appState.sessions.keys.sorted().first,
           let session = appState.sessions[sid] {
            statusFeedback = "Dispatched to \(session.source) terminal!"
            TerminalActivator.activate(session: session, sessionId: sid)

            // Copy to pasteboard so user can also ⌘V instantly
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)

            // Optional AppleScript keystroke
            let script = """
            tell application "System Events"
                keystroke "\(text.replacingOccurrences(of: "\"", with: "\\\""))"
                key code 36
            end tell
            """
            _ = AppleScriptRunner.osascript.evaluate(script, 1.0)
        } else {
            // General clipboard copy
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            statusFeedback = "Copied to clipboard!"
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
            withAnimation(NotchAnimation.close) {
                appState.surface = .collapsed
            }
        }
    }
}
