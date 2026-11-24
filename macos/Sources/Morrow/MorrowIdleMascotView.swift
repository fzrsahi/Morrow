import SwiftUI

/// The idle notch mascot for #44. Agents may be switched off for non-agentic
/// users; the app still needs a living mark in the notch, so this draws the
/// Morrow app-icon face (dark pill, two orange eyes) with a slow blink and bob.
struct MorrowIdleMascotView: View {
    let status: MascotAgentStatus
    var size: CGFloat = 27

    private let orange = Color(red: 0.96, green: 0.65, blue: 0.14)

    var body: some View {
        MascotTimeline(interval: 0.1) { t in
            Canvas { context, canvasSize in
                let scale = canvasSize.width / 16
                let bob = sin(t * 2.4) * 0.5
                let blink = t.truncatingRemainder(dividingBy: 3.6) < 0.16
                let eyeHeight: CGFloat = blink ? 0.6 : 2

                let tile = CGRect(
                    x: 0,
                    y: bob * scale,
                    width: canvasSize.width,
                    height: canvasSize.height
                )
                context.fill(
                    Path(roundedRect: tile, cornerRadius: scale * 3.5, style: .continuous),
                    with: .color(Color(white: 0.92))
                )

                let pill = CGRect(
                    x: tile.minX + scale * 3,
                    y: tile.minY + scale * 6,
                    width: scale * 10,
                    height: scale * 4
                )
                context.fill(
                    Path(roundedRect: pill, cornerRadius: scale * 2, style: .continuous),
                    with: .color(Color(white: 0.12))
                )

                let eyeY = pill.minY + (pill.height - eyeHeight * scale) / 2
                for eyeX in [5, 9] {
                    context.fill(
                        Path(CGRect(
                            x: tile.minX + scale * CGFloat(eyeX),
                            y: eyeY,
                            width: scale * 2,
                            height: eyeHeight * scale
                        )),
                        with: .color(orange)
                    )
                }
                if !blink {
                    for pupilX in [6, 10] {
                        context.fill(
                            Path(CGRect(
                                x: tile.minX + scale * CGFloat(pupilX),
                                y: eyeY + scale * 0.5,
                                width: scale,
                                height: scale
                            )),
                            with: .color(.white)
                        )
                    }
                }
            }
        }
        .frame(width: size, height: size)
    }
}

/// One place for the idle-mascot decision so the notch and its tests agree.
enum IdleMascotSource {
    static let morrow = "morrow"

    static func resolved(agentsEnabled: Bool, defaultSource: String) -> String {
        guard agentsEnabled else { return morrow }
        let preferred = defaultSource.trimmingCharacters(in: .whitespacesAndNewlines)
        return preferred.isEmpty ? SettingsDefaults.defaultSource : preferred
    }
}
