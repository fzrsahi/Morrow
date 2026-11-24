import SwiftUI

public struct DioramaSkyPalette {
    private struct RGB {
        var r: Double
        var g: Double
        var b: Double
        var color: Color { Color(red: r, green: g, blue: b) }
    }

    private static func mix(_ a: RGB, _ b: RGB, _ t: Double) -> RGB {
        RGB(r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t)
    }

    private static let keys: [(h: Double, top: RGB, bottom: RGB)] = [
        (0.0,  RGB(r: 0.07, g: 0.07, b: 0.18), RGB(r: 0.16, g: 0.13, b: 0.28)),
        (4.5,  RGB(r: 0.08, g: 0.08, b: 0.22), RGB(r: 0.22, g: 0.16, b: 0.34)),
        (5.5,  RGB(r: 0.16, g: 0.20, b: 0.42), RGB(r: 0.95, g: 0.55, b: 0.40)),
        (7.5,  RGB(r: 0.45, g: 0.72, b: 0.95), RGB(r: 0.98, g: 0.85, b: 0.62)),
        (12.0, RGB(r: 0.30, g: 0.62, b: 0.98), RGB(r: 0.62, g: 0.84, b: 1.00)),
        (16.0, RGB(r: 0.40, g: 0.55, b: 0.85), RGB(r: 1.00, g: 0.78, b: 0.45)),
        (18.3, RGB(r: 0.30, g: 0.22, b: 0.45), RGB(r: 1.00, g: 0.42, b: 0.22)),
        (19.6, RGB(r: 0.11, g: 0.10, b: 0.26), RGB(r: 0.26, g: 0.18, b: 0.38)),
        (21.5, RGB(r: 0.09, g: 0.09, b: 0.22), RGB(r: 0.20, g: 0.16, b: 0.32)),
        (24.0, RGB(r: 0.07, g: 0.07, b: 0.18), RGB(r: 0.16, g: 0.13, b: 0.28)),
    ]

    public static func colors(_ hour: Double) -> (Color, Color) {
        let h = hour.truncatingRemainder(dividingBy: 24)
        for i in 1..<keys.count where h <= keys[i].h {
            let a = keys[i - 1], b = keys[i]
            let t = (h - a.h) / (b.h - a.h)
            return (mix(a.top, b.top, t).color, mix(a.bottom, b.bottom, t).color)
        }
        return (keys[0].top.color, keys[0].bottom.color)
    }

    public static func pixelBands(_ hour: Double) -> [Color] {
        let h = hour.truncatingRemainder(dividingBy: 24)
        if h >= 4.5 && h < 6.0 {
            // Subuh / Fajar: Deep Indigo -> Twilight Violet -> Deep Rose -> Dawn Amber
            return [
                Color(red: 0.05, green: 0.06, blue: 0.15),
                Color(red: 0.18, green: 0.08, blue: 0.30),
                Color(red: 0.45, green: 0.12, blue: 0.35),
                Color(red: 0.85, green: 0.42, blue: 0.28)
            ]
        } else if h >= 6.0 && h < 11.5 {
            // Pagi / Morning: Vivid Azure -> Retro Cyan Sky -> Bright Cyan -> Soft Warm Horizon
            return [
                Color(red: 0.12, green: 0.35, blue: 0.82),
                Color(red: 0.20, green: 0.55, blue: 0.95),
                Color(red: 0.40, green: 0.75, blue: 0.98),
                Color(red: 0.82, green: 0.92, blue: 0.98)
            ]
        } else if h >= 11.5 && h < 15.0 {
            // Dzuhur / Midday: Royal 8-bit Blue -> Sky Blue -> Cyan Horizon
            return [
                Color(red: 0.10, green: 0.30, blue: 0.78),
                Color(red: 0.18, green: 0.48, blue: 0.92),
                Color(red: 0.32, green: 0.70, blue: 0.96),
                Color(red: 0.65, green: 0.88, blue: 0.98)
            ]
        } else if h >= 15.0 && h < 17.5 {
            // Ashar / Late Afternoon: Azure -> Warm Amber -> Golden Ochre -> Warm Peach
            return [
                Color(red: 0.15, green: 0.32, blue: 0.75),
                Color(red: 0.55, green: 0.40, blue: 0.65),
                Color(red: 0.88, green: 0.55, blue: 0.25),
                Color(red: 0.98, green: 0.82, blue: 0.50)
            ]
        } else if h >= 17.5 && h < 19.3 {
            // Maghrib / Sunset: Deep Purple -> Crimson Red -> Sunset Orange -> Golden Horizon
            return [
                Color(red: 0.18, green: 0.05, blue: 0.28),
                Color(red: 0.55, green: 0.10, blue: 0.22),
                Color(red: 0.90, green: 0.35, blue: 0.12),
                Color(red: 0.98, green: 0.72, blue: 0.20)
            ]
        } else {
            // Isya / Midnight: Pitch Void -> Deep Navy -> Midnight Slate -> Horizon Night
            return [
                Color(red: 0.02, green: 0.03, blue: 0.08),
                Color(red: 0.05, green: 0.07, blue: 0.16),
                Color(red: 0.09, green: 0.12, blue: 0.24),
                Color(red: 0.14, green: 0.18, blue: 0.32)
            ]
        }
    }

    public static func nightFactor(_ hour: Double) -> Double {
        let h = hour.truncatingRemainder(dividingBy: 24)
        if h < 4.4 || h > 19.8 { return 1 }
        if h < 6.0 { return (6.0 - h) / 1.6 }
        if h > 19.1 { return (h - 19.1) / 0.7 }
        return 0
    }

    public static func windowsLit(_ hour: Double) -> Bool { hour >= 17.0 || hour < 5.6 }
}

/// 138pt living procedural diorama canvas with sky gradient, sun/moon arc, drifting clouds, stars, and skyline.
public struct DioramaSkyView: View {
    public let hour: Double
    public let hijriDay: Int
    public var sunriseHour: Double = 5.75
    public var sunsetHour: Double = 18.3
    public var pixel: Bool = false
    public var skyline: Bool = true
    public var skylineHeight: CGFloat = 0.12

    private struct Star {
        let x: Double
        let y: Double
        let isCross: Bool
        let brightness: Double
    }

    private static let stars: [Star] = [
        Star(x: 0.27, y: 0.26, isCross: false, brightness: 0.55),
        Star(x: 0.34, y: 0.25, isCross: false, brightness: 0.70),
        Star(x: 0.42, y: 0.36, isCross: false, brightness: 0.65),
        Star(x: 0.56, y: 0.23, isCross: false, brightness: 0.70),
        Star(x: 0.65, y: 0.29, isCross: true,  brightness: 0.90),
        Star(x: 0.74, y: 0.22, isCross: false, brightness: 0.75),
        Star(x: 0.85, y: 0.24, isCross: false, brightness: 0.65),
    ]

    private static let spritePalette: [Character: Color] = [
        "W": Color(red: 1.00, green: 0.97, blue: 0.84),
        "Y": Color(red: 1.00, green: 0.83, blue: 0.28),
        "O": Color(red: 1.00, green: 0.55, blue: 0.20),
        "M": Color(red: 0.96, green: 0.96, blue: 1.00),
        "L": Color(red: 0.72, green: 0.75, blue: 1.00),
        "D": Color(red: 0.44, green: 0.47, blue: 0.79),
        "S": Color(red: 1.00, green: 0.90, blue: 0.50),
    ]

    private static let moonSprite: [String] = [
        "................",
        ".......MM.......",
        ".....MMLL.......",
        "....MLLD....S...",
        "...MLDD....SSS..",
        "...MLD......S...",
        "...MLD..........",
        "...MLDD.........",
        "....MLLD.....S..",
        ".....MMLL.......",
        ".......MM.......",
        "................",
        "................",
        "................",
        "................",
        "................",
    ]

    private static let sunSprite: [String] = [
        ".......YY.......",
        ".......YY.......",
        "..Y..........Y..",
        "...Y..WWWW..Y...",
        ".....WYYYYW.....",
        "....WYYYYYYW....",
        "..Y.YYWWWWYY.Y..",
        "YY..YYWWWWYY..YY",
        "....YYYYYYYY....",
        "....OYYYYYYO....",
        "...Y..OOOO..Y...",
        "..Y..........Y..",
        ".......OO.......",
        ".......OO.......",
        "................",
        "................",
    ]

    public init(
        hour: Double,
        hijriDay: Int,
        sunriseHour: Double = 5.75,
        sunsetHour: Double = 18.3,
        pixel: Bool = false,
        skyline: Bool = true,
        skylineHeight: CGFloat = 0.12
    ) {
        self.hour = hour
        self.hijriDay = hijriDay
        self.sunriseHour = sunriseHour
        self.sunsetHour = sunsetHour
        self.pixel = pixel
        self.skyline = skyline
        self.skylineHeight = skylineHeight
    }

    public var body: some View {
        Canvas { ctx, size in
            let cell: CGFloat = pixel ? 3.0 : 0
            func snap(_ v: CGFloat) -> CGFloat { cell > 0 ? (v / cell).rounded() * cell : v }

            if pixel {
                // ── 1. AUTHENTIC 8-BIT STEPPED SKY WITH CHECKERBOARD DITHERING ──
                let bands = DioramaSkyPalette.pixelBands(hour)
                let bandCount = CGFloat(bands.count)
                let bandH = size.height / bandCount

                for i in 0..<bands.count {
                    let y0 = snap(CGFloat(i) * bandH)
                    let y1 = snap(CGFloat(i + 1) * bandH)
                    let rect = CGRect(x: 0, y: y0, width: size.width, height: y1 - y0)
                    ctx.fill(Path(rect), with: .color(bands[i]))

                    // Dither transition to next band
                    if i < bands.count - 1 {
                        let ditherH: CGFloat = 6.0
                        let ditherY = y1 - (ditherH / 2.0)
                        let nextColor = bands[i + 1]
                        var dx: CGFloat = 0
                        while dx < size.width {
                            var dy: CGFloat = 0
                            while dy < ditherH {
                                let px = snap(dx)
                                let py = snap(ditherY + dy)
                                if (Int(px / 3.0) + Int(py / 3.0)) % 2 == 0 {
                                    ctx.fill(Path(CGRect(x: px, y: py, width: 3.0, height: 3.0)), with: .color(nextColor))
                                }
                                dy += 3.0
                            }
                            dx += 3.0
                        }
                    }
                }
            } else {
                // ── 1. MODERN SMOOTH LINEAR GRADIENT ──
                let (top, bottom) = DioramaSkyPalette.colors(hour)
                ctx.fill(Path(CGRect(origin: .zero, size: size)),
                         with: .linearGradient(Gradient(colors: [top, bottom]),
                                               startPoint: .zero, endPoint: CGPoint(x: 0, y: size.height)))
            }

            let night = DioramaSkyPalette.nightFactor(hour)
            if night > 0.05 {
                for star in Self.stars {
                    let cx = snap(star.x * size.width)
                    let cy = snap(star.y * size.height)
                    let a = night * star.brightness
                    if pixel {
                        // 8-bit stepped pixel stars & diamond crosses
                        if star.isCross {
                            ctx.fill(Path(CGRect(x: cx, y: cy - 3, width: 3, height: 9)), with: .color(.white.opacity(a)))
                            ctx.fill(Path(CGRect(x: cx - 3, y: cy, width: 9, height: 3)), with: .color(.white.opacity(a)))
                            ctx.fill(Path(CGRect(x: cx, y: cy, width: 3, height: 3)), with: .color(Color(red: 1.0, green: 0.95, blue: 0.7).opacity(a)))
                        } else {
                            ctx.fill(Path(CGRect(x: cx, y: cy, width: 3, height: 3)), with: .color(.white.opacity(a)))
                        }
                    } else {
                        if star.isCross {
                            ctx.fill(Path(CGRect(x: cx - 1, y: cy - 1, width: 2, height: 2)), with: .color(.white.opacity(a)))
                            ctx.fill(Path(CGRect(x: cx - 2, y: cy, width: 5, height: 1)), with: .color(.white.opacity(a * 0.75)))
                            ctx.fill(Path(CGRect(x: cx, y: cy - 2, width: 1, height: 5)), with: .color(.white.opacity(a * 0.75)))
                        } else {
                            ctx.fill(Path(ellipseIn: CGRect(x: cx - 1.2, y: cy - 1.2, width: 2.4, height: 2.4)), with: .color(.white.opacity(a)))
                        }
                    }
                }
            }

            // Elevated horizon ledge (74% height) so mosque & sun clear the timeline labels below
            let baseSkylineY = skyline ? (size.height * 0.74) : (size.height * 0.95)
            let h = hour.truncatingRemainder(dividingBy: 24)
            let sunrise = sunriseHour
            let sunset = sunsetHour
            let nextSunrise = sunrise + 24.0
            let isDay = h >= (sunrise - 0.15) && h <= (sunset + 0.25)

            let p: Double = {
                if isDay {
                    return max(0.0, min(1.0, (h - sunrise) / max(0.1, sunset - sunrise)))
                } else {
                    let hh = h < sunrise ? h + 24.0 : h
                    return max(0.08, min(0.92, (hh - sunset) / max(0.1, nextSunrise - sunset)))
                }
            }()

            let zenithY = size.height * 0.32
            let cx = snap(size.width * (0.28 + 0.44 * p))
            let cy: CGFloat = {
                if isDay {
                    return snap(baseSkylineY - (baseSkylineY - zenithY) * sin(.pi * p))
                } else {
                    return snap((baseSkylineY - 12) - (baseSkylineY - 12 - zenithY) * sin(.pi * p))
                }
            }()
            let isHorizonSun = isDay && (p <= 0.12 || p >= 0.88)

            // Clip celestial body to sky above horizon so horizon suns are cleanly half-submerged
            ctx.drawLayer { skyLayer in
                skyLayer.clip(to: Path(CGRect(x: 0, y: 0, width: size.width, height: baseSkylineY)))

                if pixel {
                    let rows = isDay ? Self.sunSprite : Self.moonSprite
                    let cellSize: CGFloat = 2.5
                    let total = 16.0 * cellSize
                    let x0 = (cx - total / 2.0).rounded()
                    let y0 = (cy - total / 2.0).rounded()
                    for (ry, row) in rows.enumerated() {
                        for (rx, ch) in row.enumerated() {
                            guard let col = Self.spritePalette[ch] else { continue }
                            let rect = CGRect(
                                x: x0 + CGFloat(rx) * cellSize,
                                y: y0 + CGFloat(ry) * cellSize,
                                width: cellSize,
                                height: cellSize
                            )
                            skyLayer.fill(Path(rect), with: .color(col))
                        }
                    }
                } else {
                    if isDay {
                        let r: CGFloat = isHorizonSun ? 20 : 16
                        let sunCol = isHorizonSun
                            ? Color(red: 1.0, green: 0.56, blue: 0.24)
                            : ((h >= (sunset - 2.8) && h < sunset) ? Color(red: 1.0, green: 0.58, blue: 0.20) : Color(red: 1.0, green: 0.92, blue: 0.45))
                        let glowAlpha = isHorizonSun ? 0.22 : 0.18
                        skyLayer.fill(
                            Path(ellipseIn: CGRect(x: cx - r * 1.75, y: cy - r * 1.75, width: r * 3.5, height: r * 3.5)),
                            with: .color(sunCol.opacity(glowAlpha))
                        )
                        skyLayer.fill(
                            Path(ellipseIn: CGRect(x: cx - r, y: cy - r, width: r * 2, height: r * 2)),
                            with: .color(sunCol)
                        )
                    } else {
                        let r: CGFloat = 16
                        skyLayer.drawLayer { moonLayer in
                            moonLayer.fill(
                                Path(ellipseIn: CGRect(x: cx - r, y: cy - r, width: r * 2, height: r * 2)),
                                with: .color(Color(red: 0.96, green: 0.96, blue: 0.92))
                            )
                            moonLayer.blendMode = .destinationOut
                            let cutR = r * 0.88
                            let cutX = cx + r * 0.48
                            let cutY = cy - r * 0.12
                            moonLayer.fill(
                                Path(ellipseIn: CGRect(x: cutX - cutR, y: cutY - cutR, width: cutR * 2, height: cutR * 2)),
                                with: .color(.black)
                            )
                        }
                    }
                }
            }

            // Clouds (drift with the hour)
            let cloudColor = night > 0.3
                ? Color(red: 0.08, green: 0.07, blue: 0.16).opacity(0.40)
                : Color.white.opacity(0.35 * (1.0 - night))

            let clouds: [(CGFloat, CGFloat, CGFloat, CGFloat)] = [
                (0.44, 0.22, 48.0, 10.0),
                (0.68, 0.30, 56.0, 11.0),
            ]
            for (baseX, baseY, cw, ch) in clouds {
                let cx = (baseX * size.width + CGFloat(hour) * 3).truncatingRemainder(dividingBy: size.width + 80) - 40
                let cy = baseY * size.height
                if pixel {
                    // 8-bit stepped blocky pixel cloud
                    let px = snap(cx)
                    let py = snap(cy)
                    let pw = snap(cw)
                    let ph = snap(ch)
                    ctx.fill(Path(CGRect(x: px + 4, y: py, width: pw - 8, height: ph)), with: .color(cloudColor))
                    ctx.fill(Path(CGRect(x: px, y: py + 3, width: pw, height: ph - 3)), with: .color(cloudColor))
                } else {
                    let rect = CGRect(x: snap(cx), y: snap(cy), width: snap(cw), height: snap(ch))
                    ctx.fill(Path(roundedRect: rect, cornerRadius: ch / 2), with: .color(cloudColor))
                }
            }

            // Skyline / Mosque Architecture (elevated on horizon ledge so timeline below stays clean)
            if skyline {
                let lit = DioramaSkyPalette.windowsLit(hour)
                let silhouetteColor = Color(red: 0.035, green: 0.035, blue: 0.08)
                let windowGold = Color(red: 1.0, green: 0.88, blue: 0.38)

                if pixel {
                    // ── AUTHENTIC 8-BIT PIXEL MOSQUE SILHOUETTE ──
                    let m1X = snap(size.width * 0.34)
                    let m2X = snap(size.width * 0.66)
                    let mH: CGFloat = snap(size.height * 0.25)

                    for mx in [m1X, m2X] {
                        ctx.fill(Path(CGRect(x: mx - 5, y: baseSkylineY - mH, width: 10, height: mH)), with: .color(silhouetteColor))
                        ctx.fill(Path(CGRect(x: mx - 7, y: baseSkylineY - mH * 0.72, width: 14, height: 3)), with: .color(silhouetteColor))
                        ctx.fill(Path(CGRect(x: mx - 2.5, y: baseSkylineY - mH - 7, width: 5, height: 8)), with: .color(silhouetteColor))
                        ctx.fill(Path(CGRect(x: mx - 1, y: baseSkylineY - mH - 10, width: 2, height: 4)), with: .color(silhouetteColor))
                    }

                    // Central Grand Mosque Dome (Stepped 8-bit hemisphere)
                    let domeCX = snap(size.width * 0.50)
                    let domeBaseH = snap(size.height * 0.11)
                    let domeWallW = snap(min(size.width * 0.28, 110))
                    ctx.fill(Path(CGRect(x: domeCX - domeWallW / 2, y: baseSkylineY - domeBaseH, width: domeWallW, height: domeBaseH)), with: .color(silhouetteColor))

                    let steps: [(w: CGFloat, h: CGFloat)] = [
                        (domeWallW * 0.85, 5),
                        (domeWallW * 0.72, 6),
                        (domeWallW * 0.56, 6),
                        (domeWallW * 0.38, 5),
                        (domeWallW * 0.20, 4),
                        (domeWallW * 0.08, 4)
                    ]
                    var currY = baseSkylineY - domeBaseH
                    for step in steps {
                        currY -= step.h
                        ctx.fill(Path(CGRect(x: domeCX - step.w / 2, y: currY, width: step.w, height: step.h)), with: .color(silhouetteColor))
                    }
                    ctx.fill(Path(CGRect(x: domeCX - 1.5, y: currY - 6, width: 3, height: 6)), with: .color(silhouetteColor))
                    ctx.fill(Path(CGRect(x: domeCX - 3, y: currY - 8, width: 4, height: 2)), with: .color(silhouetteColor))

                    if lit {
                        for offX in [-20.0, 0.0, 20.0] as [CGFloat] {
                            let wx = snap(domeCX + offX - 2.5)
                            let wy = snap(baseSkylineY - domeBaseH * 0.70)
                            ctx.fill(Path(CGRect(x: wx, y: wy, width: 5, height: 7)), with: .color(windowGold.opacity(0.52)))
                            ctx.fill(Path(CGRect(x: wx + 1, y: wy - 2, width: 3, height: 2)), with: .color(windowGold.opacity(0.52)))
                        }
                    }

                    // Solid ground dock below horizon
                    ctx.fill(Path(CGRect(x: 0, y: baseSkylineY, width: size.width, height: size.height - baseSkylineY)), with: .color(silhouetteColor))
                } else {
                    // ── MODERN SMOOTH VECTOR MOSQUE SILHOUETTE ──
                    let modernSil = Color(red: 0.043, green: 0.047, blue: 0.086).opacity(0.92)
                    let domeCX = size.width * 0.50
                    let bH = size.height * 0.105
                    let domeR: CGFloat = 35

                    var domePath = Path()
                    domePath.addArc(
                        center: CGPoint(x: domeCX, y: baseSkylineY - bH),
                        radius: domeR,
                        startAngle: .degrees(180),
                        endAngle: .degrees(0),
                        clockwise: false
                    )
                    domePath.addLine(to: CGPoint(x: domeCX + 52, y: baseSkylineY - bH))
                    domePath.addLine(to: CGPoint(x: domeCX + 52, y: baseSkylineY))
                    domePath.addLine(to: CGPoint(x: domeCX - 52, y: baseSkylineY))
                    domePath.addLine(to: CGPoint(x: domeCX - 52, y: baseSkylineY - bH))
                    domePath.closeSubpath()
                    ctx.fill(domePath, with: .color(modernSil))

                    // Dome Finial Spire & Hilal Crescent
                    let peakY = baseSkylineY - bH - domeR
                    ctx.fill(Path(CGRect(x: domeCX - 1, y: peakY - 9, width: 2, height: 10)), with: .color(modernSil))
                    var hilal = Path()
                    hilal.addArc(
                        center: CGPoint(x: domeCX, y: peakY - 11),
                        radius: 3.2,
                        startAngle: .degrees(63),
                        endAngle: .degrees(297),
                        clockwise: false
                    )
                    ctx.stroke(hilal, with: .color(modernSil), lineWidth: 1.6)

                    // Left & Right Minarets with Shurfah Balconies & Tapered Spires
                    let mH = size.height * 0.24
                    for mx in [size.width * 0.34, size.width * 0.66] {
                        ctx.fill(Path(CGRect(x: mx - 4.5, y: baseSkylineY - mH, width: 9, height: mH)), with: .color(modernSil))
                        ctx.fill(Path(roundedRect: CGRect(x: mx - 7.5, y: baseSkylineY - mH * 0.72, width: 15, height: 3), cornerRadius: 1.5), with: .color(modernSil))
                        ctx.fill(Path(CGRect(x: mx - 3, y: baseSkylineY - mH - 6, width: 6, height: 6)), with: .color(modernSil))
                        var spire = Path()
                        spire.move(to: CGPoint(x: mx - 3.5, y: baseSkylineY - mH - 6))
                        spire.addLine(to: CGPoint(x: mx, y: baseSkylineY - mH - 14))
                        spire.addLine(to: CGPoint(x: mx + 3.5, y: baseSkylineY - mH - 6))
                        spire.closeSubpath()
                        ctx.fill(spire, with: .color(modernSil))
                    }

                    if lit {
                        for offX in [-18.0, 0.0, 18.0] as [CGFloat] {
                            let winRect = CGRect(x: domeCX + offX - 2.5, y: baseSkylineY - bH * 0.68, width: 5, height: 7.5)
                            ctx.fill(Path(roundedRect: winRect, cornerRadius: 2.5), with: .color(windowGold.opacity(0.52)))
                        }
                    }

                    // Solid ground dock below horizon
                    ctx.fill(Path(CGRect(x: 0, y: baseSkylineY, width: size.width, height: size.height - baseSkylineY)), with: .color(modernSil))
                }
            }
        }
    }
}
