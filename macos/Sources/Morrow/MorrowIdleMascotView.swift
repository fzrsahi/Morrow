import SwiftUI

/// The idle notch mascot for #44. Agents may be switched off for non-agentic
/// users; the app still needs a living mark in the notch, so this draws Pip,
/// the dawn songbird from `docs/mascot-options/pip.svg`, as transparent vector
/// art with a slow blink and bob. The SVG's dark preview backing is not drawn.
struct MorrowIdleMascotView: View {
    let status: MascotAgentStatus
    var size: CGFloat = 27

    // Pip's palette, copied from the SVG's gradient stops and fills.
    private static let gold = Color(red: 1, green: 211 / 255, blue: 122 / 255)
    private static let coral = Color(red: 240 / 255, green: 138 / 255, blue: 93 / 255)
    private static let tail = Color(red: 217 / 255, green: 111 / 255, blue: 75 / 255)
    private static let wing = Color(red: 229 / 255, green: 124 / 255, blue: 80 / 255)
    private static let beak = Color(red: 200 / 255, green: 90 / 255, blue: 60 / 255)
    private static let eye = Color(red: 42 / 255, green: 31 / 255, blue: 26 / 255)

    var body: some View {
        MascotTimeline(interval: 0.1) { t in
            Canvas { context, canvasSize in
                let scale = min(canvasSize.width, canvasSize.height) / 64
                let bob = sin(t * 2.4) * 0.5 * scale
                let blink = t.truncatingRemainder(dividingBy: 3.6) < 0.16

                func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
                    CGPoint(x: x * scale, y: y * scale + bob)
                }
                func bodyShading(from top: CGFloat, to bottom: CGFloat) -> GraphicsContext.Shading {
                    .linearGradient(
                        Gradient(colors: [Self.gold, Self.coral]),
                        startPoint: point(0, top),
                        endPoint: point(0, bottom)
                    )
                }

                // Tail: M21 44 L11 51 L23 49 Z
                var tail = Path()
                tail.move(to: point(21, 44))
                tail.addLine(to: point(11, 51))
                tail.addLine(to: point(23, 49))
                tail.closeSubpath()
                context.fill(tail, with: .color(Self.tail))

                // Body: ellipse cx=32 cy=38 rx=17 ry=15
                let body = CGRect(x: 15 * scale, y: 23 * scale + bob,
                                  width: 34 * scale, height: 30 * scale)
                context.fill(Path(ellipseIn: body), with: bodyShading(from: 23, to: 53))

                // Wings: M18 35 Q11 39 19 47 Q24 44 23 38 Z, mirrored on x=32.
                var leftWing = Path()
                leftWing.move(to: point(18, 35))
                leftWing.addQuadCurve(to: point(19, 47), control: point(11, 39))
                leftWing.addQuadCurve(to: point(23, 38), control: point(24, 44))
                leftWing.closeSubpath()
                context.fill(leftWing, with: .color(Self.wing))

                var rightWing = Path()
                rightWing.move(to: point(46, 35))
                rightWing.addQuadCurve(to: point(45, 47), control: point(53, 39))
                rightWing.addQuadCurve(to: point(41, 38), control: point(40, 44))
                rightWing.closeSubpath()
                context.fill(rightWing, with: .color(Self.wing))

                // Head: circle cx=32 cy=23 r=14
                let head = CGRect(x: 18 * scale, y: 9 * scale + bob,
                                  width: 28 * scale, height: 28 * scale)
                context.fill(Path(ellipseIn: head), with: bodyShading(from: 9, to: 37))

                // Crest: M29 10 Q32 4 35 10 Z
                var crest = Path()
                crest.move(to: point(29, 10))
                crest.addQuadCurve(to: point(35, 10), control: point(32, 4))
                crest.closeSubpath()
                context.fill(crest, with: .color(Self.gold))

                // Beak: M32 25 L27 29 L32 31 L37 29 Z
                var beak = Path()
                beak.move(to: point(32, 25))
                beak.addLine(to: point(27, 29))
                beak.addLine(to: point(32, 31))
                beak.addLine(to: point(37, 29))
                beak.closeSubpath()
                context.fill(beak, with: .color(Self.beak))

                // Eyes: r=3.2 at (26,21) and (38,21); blink flattens them.
                let eyeHeight: CGFloat = blink ? 1 : 6.4
                let eyeTop = 21 - eyeHeight / 2
                for x in [26, 38] {
                    let eye = CGRect(x: (CGFloat(x) - 3.2) * scale,
                                     y: eyeTop * scale + bob,
                                     width: 6.4 * scale,
                                     height: eyeHeight * scale)
                    context.fill(Path(ellipseIn: eye), with: .color(Self.eye))
                }
                if !blink {
                    for highlight in [CGPoint(x: 27, y: 20), CGPoint(x: 39, y: 20)] {
                        let dot = CGRect(x: (highlight.x - 1) * scale,
                                         y: (highlight.y - 1) * scale + bob,
                                         width: 2 * scale,
                                         height: 2 * scale)
                        context.fill(Path(ellipseIn: dot), with: .color(.white))
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
