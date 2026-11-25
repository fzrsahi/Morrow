import SwiftUI

/// The idle notch mascot for #44. Agents may be switched off for non-agentic
/// users; the app still needs a living mark in the notch, so this draws Nori,
/// the notch-dwelling cat from `docs/mascot-options/nori.svg`, as transparent
/// vector art with a slow blink and bob. The SVG's dark preview backing is not
/// drawn: Nori sits directly on the notch chrome and reads by its amber eyes.
struct MorrowIdleMascotView: View {
    let status: MascotAgentStatus
    var size: CGFloat = 27

    // Nori's palette, copied from the SVG's gradient stops and fills.
    private static let bodyTop = Color(red: 90 / 255, green: 95 / 255, blue: 150 / 255)
    private static let bodyBottom = Color(red: 52 / 255, green: 56 / 255, blue: 89 / 255)
    private static let tail = Color(red: 66 / 255, green: 72 / 255, blue: 120 / 255)
    private static let ear = Color(red: 82 / 255, green: 88 / 255, blue: 140 / 255)
    private static let innerEar = Color(red: 233 / 255, green: 145 / 255, blue: 106 / 255)
    private static let eye = Color(red: 1, green: 196 / 255, blue: 107 / 255)
    private static let pupil = Color(red: 32 / 255, green: 34 / 255, blue: 58 / 255)
    private static let mouth = Color(red: 185 / 255, green: 189 / 255, blue: 224 / 255)

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
                        Gradient(colors: [Self.bodyTop, Self.bodyBottom]),
                        startPoint: point(0, top),
                        endPoint: point(0, bottom)
                    )
                }

                // Tail: M47 45 Q59 41 54 29 Q52 37 45 39 Z
                var tail = Path()
                tail.move(to: point(47, 45))
                tail.addQuadCurve(to: point(54, 29), control: point(59, 41))
                tail.addQuadCurve(to: point(45, 39), control: point(52, 37))
                tail.closeSubpath()
                context.fill(tail, with: .color(Self.tail))

                // Body: ellipse cx=32 cy=41 rx=16 ry=14
                let body = CGRect(x: 16 * scale, y: 27 * scale + bob,
                                  width: 32 * scale, height: 28 * scale)
                context.fill(Path(ellipseIn: body), with: bodyShading(from: 27, to: 55))

                // Ears: M19 22 Q17 7 29 16 Q25 19 19 22 Z, mirrored on x=32.
                var leftEar = Path()
                leftEar.move(to: point(19, 22))
                leftEar.addQuadCurve(to: point(29, 16), control: point(17, 7))
                leftEar.addQuadCurve(to: point(19, 22), control: point(25, 19))
                leftEar.closeSubpath()
                context.fill(leftEar, with: .color(Self.ear))

                var rightEar = Path()
                rightEar.move(to: point(45, 22))
                rightEar.addQuadCurve(to: point(35, 16), control: point(47, 7))
                rightEar.addQuadCurve(to: point(45, 22), control: point(39, 19))
                rightEar.closeSubpath()
                context.fill(rightEar, with: .color(Self.ear))

                var leftInnerEar = Path()
                leftInnerEar.move(to: point(22, 20))
                leftInnerEar.addQuadCurve(to: point(27, 17), control: point(21, 12))
                leftInnerEar.closeSubpath()
                context.fill(leftInnerEar, with: .color(Self.innerEar))

                var rightInnerEar = Path()
                rightInnerEar.move(to: point(42, 20))
                rightInnerEar.addQuadCurve(to: point(37, 17), control: point(43, 12))
                rightInnerEar.closeSubpath()
                context.fill(rightInnerEar, with: .color(Self.innerEar))

                // Head: circle cx=32 cy=25 r=15
                let head = CGRect(x: 17 * scale, y: 10 * scale + bob,
                                  width: 30 * scale, height: 30 * scale)
                context.fill(Path(ellipseIn: head), with: bodyShading(from: 10, to: 40))

                // Eyes: amber ellipses at (26,25) and (38,25), with slit pupils.
                // Blink flattens both without removing the amber signal at 20 pt.
                let eyeHeight: CGFloat = blink ? 0.9 : 4.2
                let pupilHeight: CGFloat = blink ? 0.4 : 2.4
                for x in [26, 38] {
                    let eye = CGRect(x: (CGFloat(x) - 3.2) * scale,
                                     y: (25 - eyeHeight / 2) * scale + bob,
                                     width: 6.4 * scale,
                                     height: eyeHeight * scale)
                    context.fill(Path(ellipseIn: eye), with: .color(Self.eye))

                    let pupil = CGRect(x: (CGFloat(x) - 1.2) * scale,
                                       y: (25 - pupilHeight / 2) * scale + bob,
                                       width: 2.4 * scale,
                                       height: pupilHeight * scale)
                    context.fill(Path(ellipseIn: pupil), with: .color(Self.pupil))
                }

                // Nose: M29 32 L35 32 L32 35 Z
                var nose = Path()
                nose.move(to: point(29, 32))
                nose.addLine(to: point(35, 32))
                nose.addLine(to: point(32, 35))
                nose.closeSubpath()
                context.fill(nose, with: .color(Self.innerEar))

                // Mouth: M32 35 Q29 38 26 35 M32 35 Q35 38 38 35
                var mouth = Path()
                mouth.move(to: point(32, 35))
                mouth.addQuadCurve(to: point(26, 35), control: point(29, 38))
                mouth.move(to: point(32, 35))
                mouth.addQuadCurve(to: point(38, 35), control: point(35, 38))
                context.stroke(
                    mouth,
                    with: .color(Self.mouth),
                    style: StrokeStyle(lineWidth: 1.2 * scale, lineCap: .round)
                )
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
