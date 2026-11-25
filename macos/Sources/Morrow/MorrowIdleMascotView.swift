import SwiftUI

/// The idle notch mascot for #44. Agents may be switched off for non-agentic
/// users; the app still needs a living mark in the notch, so this draws Lumen,
/// the dawn signal companion, from the two `docs/mascot-options/lumen-*.svg`
/// vectors. The SVGs' dark preview background is not drawn. The existing
/// `islandUITheme` key picks the smooth Modern finish or the blocky Pixel one;
/// both keep a slow blink and bob on `MascotTimeline`.
struct MorrowIdleMascotView: View {
    let status: MascotAgentStatus
    var size: CGFloat = 27
    @AppStorage(SettingsKey.islandUITheme) private var islandUITheme = SettingsDefaults.islandUITheme

    private var isPixel: Bool { islandUITheme == IslandUITheme.pixel.rawValue }

    // Modern palette: indigo → blue body, glass edge and rounded glowing eyes.
    private static let bodyTop = Color(red: 143 / 255, green: 216 / 255, blue: 1)
    private static let bodyBottom = Color(red: 108 / 255, green: 123 / 255, blue: 1)
    private static let antennaDot = Color(red: 188 / 255, green: 235 / 255, blue: 1)
    private static let modernEye = Color(red: 143 / 255, green: 216 / 255, blue: 1)
    private static let modernScreen = Color(red: 22 / 255, green: 27 / 255, blue: 58 / 255)

    // Pixel palette: hard 16-bit steps in blue with cyan accents.
    private static let pixelBody = Color(red: 58 / 255, green: 111 / 255, blue: 191 / 255)
    private static let pixelBodyShade = Color(red: 42 / 255, green: 82 / 255, blue: 150 / 255)
    private static let pixelScreen = Color(red: 16 / 255, green: 20 / 255, blue: 38 / 255)
    private static let pixelScreenInset = Color(red: 24 / 255, green: 32 / 255, blue: 67 / 255)
    private static let pixelCyan = Color(red: 90 / 255, green: 209 / 255, blue: 230 / 255)
    private static let pixelCyanLight = Color(red: 155 / 255, green: 233 / 255, blue: 245 / 255)

    var body: some View {
        MascotTimeline(interval: 0.1) { t in
            Canvas { context, canvasSize in
                if self.isPixel {
                    self.drawPixel(context: &context, canvasSize: canvasSize, t: t)
                } else {
                    self.drawModern(context: &context, canvasSize: canvasSize, t: t)
                }
            }
        }
        .frame(width: size, height: size)
    }

    // MARK: - Modern: rounded body, glass highlight and soft glow

    private func drawModern(context: inout GraphicsContext, canvasSize: CGSize, t: Double) {
        let scale = min(canvasSize.width, canvasSize.height) / 64
        let bob = sin(t * 2.4) * 0.5 * scale
        let blink = t.truncatingRemainder(dividingBy: 3.6) < 0.16

        func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
            CGPoint(x: x * scale, y: y * scale + bob)
        }
        func rect(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> CGRect {
            CGRect(x: x * scale, y: y * scale + bob, width: w * scale, height: h * scale)
        }

        // Signal antenna: M32 12 L32 5, circle cx=32 cy=4 r=2.4
        var antenna = Path()
        antenna.move(to: point(32, 12))
        antenna.addLine(to: point(32, 5))
        context.stroke(
            antenna,
            with: .color(Self.modernEye),
            style: StrokeStyle(lineWidth: 2 * scale, lineCap: .round)
        )
        context.fill(Path(ellipseIn: rect(29.6, 1.6, 4.8, 4.8)), with: .color(Self.antennaDot))

        // Soft outer glow: circle cx=32 cy=35 r=21, opacity 0.16
        context.fill(
            Path(ellipseIn: rect(11, 14, 42, 42)),
            with: .color(Self.bodyBottom.opacity(0.16))
        )

        // Body: rounded rect x=13 y=18 w=38 h=34 rx=15, indigo → blue
        context.fill(
            Path(roundedRect: rect(13, 18, 38, 34), cornerRadius: 15 * scale, style: .continuous),
            with: .linearGradient(
                Gradient(colors: [Self.bodyTop, Self.bodyBottom]),
                startPoint: point(0, 18),
                endPoint: point(0, 52)
            )
        )

        // Glass highlight: M18 26 Q20 20 28 19, opacity 0.45
        var glass = Path()
        glass.move(to: point(18, 26))
        glass.addQuadCurve(to: point(28, 19), control: point(20, 20))
        context.stroke(
            glass,
            with: .color(Color.white.opacity(0.45)),
            style: StrokeStyle(lineWidth: 2 * scale, lineCap: .round)
        )

        // Screen face: x=20 y=27 w=24 h=16 rx=8
        context.fill(
            Path(roundedRect: rect(20, 27, 24, 16), cornerRadius: 8 * scale, style: .continuous),
            with: .color(Self.modernScreen)
        )

        // Glowing eyes: x=25/35 y=32 w=4 h=6 rx=2; blink flattens them.
        let eyeHeight: CGFloat = blink ? 1.2 : 6
        let eyeTop = 35 - eyeHeight / 2
        for x in [25, 35] {
            context.fill(
                Path(roundedRect: rect(CGFloat(x), eyeTop, 4, eyeHeight),
                     cornerRadius: min(2, eyeHeight / 2) * scale, style: .continuous),
                with: .color(Self.modernEye)
            )
        }

        // Tiny smile: M29 40 Q32 42 35 40
        var smile = Path()
        smile.move(to: point(29, 40))
        smile.addQuadCurve(to: point(35, 40), control: point(32, 42))
        context.stroke(
            smile,
            with: .color(Self.modernEye.opacity(0.8)),
            style: StrokeStyle(lineWidth: 1.2 * scale, lineCap: .round)
        )
    }

    // MARK: - Pixel: hard-stepped body, inset screen and square eyes

    private func drawPixel(context: inout GraphicsContext, canvasSize: CGSize, t: Double) {
        let scale = min(canvasSize.width, canvasSize.height) / 64
        let bob = sin(t * 2.4) * 0.5 * scale
        let blink = t.truncatingRemainder(dividingBy: 3.6) < 0.16

        func fill(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ color: Color) {
            let tile = CGRect(x: x * scale, y: y * scale + bob,
                              width: w * scale, height: h * scale)
            context.fill(Path(tile), with: .color(color))
        }

        // 16-bit antenna
        fill(30, 4, 4, 12, Self.pixelCyan)
        fill(28, 2, 8, 6, Self.pixelCyanLight)

        // Chunky body with hard steps and shaded edges
        fill(16, 16, 32, 4, Self.pixelBody)
        fill(12, 20, 40, 30, Self.pixelBody)
        fill(16, 50, 32, 4, Self.pixelBody)
        fill(12, 20, 4, 30, Self.pixelBodyShade)
        fill(48, 20, 4, 30, Self.pixelBodyShade)

        // Inset screen
        fill(20, 26, 24, 18, Self.pixelScreen)
        fill(22, 28, 20, 14, Self.pixelScreenInset)

        // Square pixel eyes: blink folds them into single rows
        let eyeHeight: CGFloat = blink ? 2 : 8
        let eyeTop: CGFloat = blink ? 34 : 31
        fill(25, eyeTop, 5, eyeHeight, Self.pixelCyan)
        fill(35, eyeTop, 5, eyeHeight, Self.pixelCyan)

        // Pixel grin
        fill(28, 41, 9, 2, Self.pixelCyanLight)
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
