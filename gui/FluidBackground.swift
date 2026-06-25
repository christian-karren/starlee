import AppKit

struct FluidBackgroundSettings: Equatable {
    static let defaultPixelColor = "#062F64"
    static let defaultBackgroundColor = "#F9E4B6"

    // Background engine: "pixel-dither", "flow", "aurora", "dither", "glass".
    var kind: String
    // pixelColor doubles as "navy" and backgroundColor as "cream" for the
    // aurora/dither/glass engines; black/white are used only by those.
    var pixelColor: String
    var backgroundColor: String
    var black: String
    var white: String
    var pixelSize: Double
    var threshold: Double
    var speed: Double
    var zoom: Double
    // Flow-engine parameters.
    var flowFinish: String
    // Shared procedural seed (flow / aurora / dither / glass).
    var flowSeed: Double
    // Aurora-engine parameters.
    var auroraIntensity: Double
    // Dither-engine parameters.
    var ditherDotSize: Double
    var ditherContrast: Double
    var ditherNavyBuffer: Double
    // Glass-engine parameters.
    var glassMode: String
    var glassPanes: Double
    var glassSoftness: Double
    var glassBrightness: Double
    var glassRefraction: Double

    init(
        pixelColor: String,
        backgroundColor: String,
        pixelSize: Double,
        threshold: Double,
        speed: Double,
        zoom: Double,
        kind: String = "pixel-dither",
        black: String = "#000000",
        white: String = "#FFFFFF",
        flowFinish: String = "soft",
        flowSeed: Double = 0.42,
        auroraIntensity: Double = 0.55,
        ditherDotSize: Double = 6,
        ditherContrast: Double = 1.3,
        ditherNavyBuffer: Double = 1.4,
        glassMode: String = "panes",
        glassPanes: Double = 18,
        glassSoftness: Double = 14,
        glassBrightness: Double = 1.0,
        glassRefraction: Double = 0.02
    ) {
        self.kind = kind
        self.pixelColor = pixelColor
        self.backgroundColor = backgroundColor
        self.black = black
        self.white = white
        self.pixelSize = pixelSize
        self.threshold = threshold
        self.speed = speed
        self.zoom = zoom
        self.flowFinish = flowFinish
        self.flowSeed = flowSeed
        self.auroraIntensity = auroraIntensity
        self.ditherDotSize = ditherDotSize
        self.ditherContrast = ditherContrast
        self.ditherNavyBuffer = ditherNavyBuffer
        self.glassMode = glassMode
        self.glassPanes = glassPanes
        self.glassSoftness = glassSoftness
        self.glassBrightness = glassBrightness
        self.glassRefraction = glassRefraction
    }

    /// Decodes settings posted from the web Settings page (the webPayload shape).
    init(payload: [String: Any]) {
        let d = FluidBackgroundSettings.default
        func str(_ key: String, _ fallback: String) -> String { payload[key] as? String ?? fallback }
        func num(_ key: String, _ fallback: Double) -> Double {
            if let n = payload[key] as? NSNumber { return n.doubleValue }
            if let v = payload[key] as? Double { return v }
            return fallback
        }
        self.init(
            pixelColor: str("pixelColor", d.pixelColor),
            backgroundColor: str("backgroundColor", d.backgroundColor),
            pixelSize: num("pixelSize", d.pixelSize),
            threshold: num("threshold", d.threshold),
            speed: num("speed", d.speed),
            zoom: num("zoom", d.zoom),
            kind: str("kind", d.kind),
            black: str("black", d.black),
            white: str("white", d.white),
            flowFinish: str("flowFinish", d.flowFinish),
            flowSeed: num("flowSeed", d.flowSeed),
            auroraIntensity: num("auroraIntensity", d.auroraIntensity),
            ditherDotSize: num("ditherDotSize", d.ditherDotSize),
            ditherContrast: num("ditherContrast", d.ditherContrast),
            ditherNavyBuffer: num("ditherNavyBuffer", d.ditherNavyBuffer),
            glassMode: str("glassMode", d.glassMode),
            glassPanes: num("glassPanes", d.glassPanes),
            glassSoftness: num("glassSoftness", d.glassSoftness),
            glassBrightness: num("glassBrightness", d.glassBrightness),
            glassRefraction: num("glassRefraction", d.glassRefraction)
        )
    }

    static let `default` = FluidBackgroundSettings(
        pixelColor: defaultPixelColor,
        backgroundColor: defaultBackgroundColor,
        pixelSize: 6,
        threshold: 0.31,
        speed: 0.02,
        zoom: 4.8
    )

    var isFlow: Bool { kind == "flow" }

    var webPayload: [String: Any] {
        [
            "kind": kind,
            "pixelColor": pixelColor,
            "backgroundColor": backgroundColor,
            "black": black,
            "white": white,
            "pixelSize": pixelSize,
            "threshold": threshold,
            "speed": speed,
            "zoom": zoom,
            "flowFinish": flowFinish,
            "flowSeed": flowSeed,
            "auroraIntensity": auroraIntensity,
            "ditherDotSize": ditherDotSize,
            "ditherContrast": ditherContrast,
            "ditherNavyBuffer": ditherNavyBuffer,
            "glassMode": glassMode,
            "glassPanes": glassPanes,
            "glassSoftness": glassSoftness,
            "glassBrightness": glassBrightness,
            "glassRefraction": glassRefraction
        ]
    }

    var webPayloadJSON: String {
        guard
            JSONSerialization.isValidJSONObject(webPayload),
            let data = try? JSONSerialization.data(withJSONObject: webPayload, options: []),
            let json = String(data: data, encoding: .utf8)
        else {
            return ##"{"kind":"pixel-dither","pixelColor":"#062F64","backgroundColor":"#F9E4B6","pixelSize":6,"threshold":0.31,"speed":0.02,"zoom":4.8}"##
        }
        return json
    }

    static func rgbInteger(_ hex: String) -> Int {
        let clean = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")).uppercased()
        guard clean.count == 6, let value = Int(clean, radix: 16) else { return 0 }
        return value
    }

    static func hex(from color: NSColor) -> String {
        let converted = color.usingColorSpace(.sRGB) ?? color
        let red = max(0, min(255, Int(round(converted.redComponent * 255))))
        let green = max(0, min(255, Int(round(converted.greenComponent * 255))))
        let blue = max(0, min(255, Int(round(converted.blueComponent * 255))))
        return String(format: "#%02X%02X%02X", red, green, blue)
    }

    static func color(from hex: String) -> NSColor {
        let value = rgbInteger(hex)
        return NSColor(
            calibratedRed: CGFloat((value >> 16) & 0xFF) / 255,
            green: CGFloat((value >> 8) & 0xFF) / 255,
            blue: CGFloat(value & 0xFF) / 255,
            alpha: 1
        )
    }

}

struct FluidBackgroundLook {
    let name: String
    let settings: FluidBackgroundSettings
}

enum FluidBackgroundLooks {
    // Brand palette shared by the aurora / dither / glass engines.
    static let navy = "#13284B"
    static let cream = "#F2E3B6"

    static let all: [FluidBackgroundLook] = [
        FluidBackgroundLook(
            name: "Navy Cream ·175",
            settings: FluidBackgroundSettings(
                pixelColor: "#062F64",
                backgroundColor: "#F9E4B6",
                pixelSize: 6,
                threshold: 0.175,
                speed: 0.02,
                zoom: 4.8
            )
        ),
        FluidBackgroundLook(
            name: "Navy Cream ·366",
            settings: FluidBackgroundSettings(
                pixelColor: "#062F64",
                backgroundColor: "#F9E4B6",
                pixelSize: 6,
                threshold: 0.366,
                speed: 0.02,
                zoom: 4.8
            )
        ),
        FluidBackgroundLook(
            name: "Ribbon",
            settings: FluidBackgroundSettings(
                pixelColor: "#102A57",
                backgroundColor: "#F2E0AE",
                pixelSize: 6,
                threshold: 0.31,
                speed: 0.018,
                zoom: 4.6,
                kind: "flow",
                flowFinish: "sharp",
                flowSeed: 0.42
            )
        ),
        FluidBackgroundLook(
            name: "Aurora",
            settings: FluidBackgroundSettings(
                pixelColor: navy,
                backgroundColor: cream,
                pixelSize: 6,
                threshold: 0.31,
                speed: 0.7,
                zoom: 4.8,
                kind: "aurora",
                flowSeed: 0.61,
                auroraIntensity: 0.55
            )
        ),
        FluidBackgroundLook(
            name: "Dither",
            settings: FluidBackgroundSettings(
                pixelColor: navy,
                backgroundColor: cream,
                pixelSize: 6,
                threshold: 0.31,
                speed: 0.005,
                zoom: 4.8,
                kind: "dither",
                flowSeed: 0.5,
                ditherDotSize: 6,
                ditherContrast: 1.3,
                ditherNavyBuffer: 1.4
            )
        ),
        FluidBackgroundLook(
            name: "Glass",
            settings: FluidBackgroundSettings(
                pixelColor: navy,
                backgroundColor: cream,
                pixelSize: 6,
                threshold: 0.31,
                speed: 0.004,
                zoom: 4.8,
                kind: "glass",
                flowSeed: 0.78,
                glassMode: "panes",
                glassPanes: 18,
                glassSoftness: 14,
                glassBrightness: 1.0,
                glassRefraction: 0.02
            )
        )
    ]
}

final class FluidBackgroundSettingsStore {
    private enum Key {
        static let kind = "StarleeFluidKind"
        static let pixelColor = "StarleeFluidPixelColor"
        static let backgroundColor = "StarleeFluidBackgroundColor"
        static let black = "StarleeFluidBlack"
        static let white = "StarleeFluidWhite"
        static let pixelSize = "StarleeFluidPixelSize"
        static let threshold = "StarleeFluidThreshold"
        static let speed = "StarleeFluidSpeed"
        static let zoom = "StarleeFluidZoom"
        static let flowFinish = "StarleeFluidFlowFinish"
        static let flowSeed = "StarleeFluidFlowSeed"
        static let auroraIntensity = "StarleeFluidAuroraIntensity"
        static let ditherDotSize = "StarleeFluidDitherDotSize"
        static let ditherContrast = "StarleeFluidDitherContrast"
        static let ditherNavyBuffer = "StarleeFluidDitherNavyBuffer"
        static let glassMode = "StarleeFluidGlassMode"
        static let glassPanes = "StarleeFluidGlassPanes"
        static let glassSoftness = "StarleeFluidGlassSoftness"
        static let glassBrightness = "StarleeFluidGlassBrightness"
        static let glassRefraction = "StarleeFluidGlassRefraction"
        static let engineVersion = "StarleePixelDitherEngineVersion"
    }

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func load() -> FluidBackgroundSettings {
        let fallback = FluidBackgroundSettings.default
        if defaults.integer(forKey: Key.engineVersion) < 1 {
            save(fallback)
            return fallback
        }
        let settings = FluidBackgroundSettings(
            pixelColor: defaults.string(forKey: Key.pixelColor) ?? fallback.pixelColor,
            backgroundColor: defaults.string(forKey: Key.backgroundColor) ?? fallback.backgroundColor,
            pixelSize: value(for: Key.pixelSize, fallback: fallback.pixelSize),
            threshold: value(for: Key.threshold, fallback: fallback.threshold),
            speed: value(for: Key.speed, fallback: fallback.speed),
            zoom: value(for: Key.zoom, fallback: fallback.zoom),
            kind: defaults.string(forKey: Key.kind) ?? fallback.kind,
            black: defaults.string(forKey: Key.black) ?? fallback.black,
            white: defaults.string(forKey: Key.white) ?? fallback.white,
            flowFinish: defaults.string(forKey: Key.flowFinish) ?? fallback.flowFinish,
            flowSeed: value(for: Key.flowSeed, fallback: fallback.flowSeed),
            auroraIntensity: value(for: Key.auroraIntensity, fallback: fallback.auroraIntensity),
            ditherDotSize: value(for: Key.ditherDotSize, fallback: fallback.ditherDotSize),
            ditherContrast: value(for: Key.ditherContrast, fallback: fallback.ditherContrast),
            ditherNavyBuffer: value(for: Key.ditherNavyBuffer, fallback: fallback.ditherNavyBuffer),
            glassMode: defaults.string(forKey: Key.glassMode) ?? fallback.glassMode,
            glassPanes: value(for: Key.glassPanes, fallback: fallback.glassPanes),
            glassSoftness: value(for: Key.glassSoftness, fallback: fallback.glassSoftness),
            glassBrightness: value(for: Key.glassBrightness, fallback: fallback.glassBrightness),
            glassRefraction: value(for: Key.glassRefraction, fallback: fallback.glassRefraction)
        )
        if isLegacyDefault(settings) {
            save(fallback)
            return fallback
        }
        return settings
    }

    func save(_ settings: FluidBackgroundSettings) {
        defaults.set(settings.kind, forKey: Key.kind)
        defaults.set(settings.pixelColor, forKey: Key.pixelColor)
        defaults.set(settings.backgroundColor, forKey: Key.backgroundColor)
        defaults.set(settings.black, forKey: Key.black)
        defaults.set(settings.white, forKey: Key.white)
        defaults.set(settings.pixelSize, forKey: Key.pixelSize)
        defaults.set(settings.threshold, forKey: Key.threshold)
        defaults.set(settings.speed, forKey: Key.speed)
        defaults.set(settings.zoom, forKey: Key.zoom)
        defaults.set(settings.flowFinish, forKey: Key.flowFinish)
        defaults.set(settings.flowSeed, forKey: Key.flowSeed)
        defaults.set(settings.auroraIntensity, forKey: Key.auroraIntensity)
        defaults.set(settings.ditherDotSize, forKey: Key.ditherDotSize)
        defaults.set(settings.ditherContrast, forKey: Key.ditherContrast)
        defaults.set(settings.ditherNavyBuffer, forKey: Key.ditherNavyBuffer)
        defaults.set(settings.glassMode, forKey: Key.glassMode)
        defaults.set(settings.glassPanes, forKey: Key.glassPanes)
        defaults.set(settings.glassSoftness, forKey: Key.glassSoftness)
        defaults.set(settings.glassBrightness, forKey: Key.glassBrightness)
        defaults.set(settings.glassRefraction, forKey: Key.glassRefraction)
        defaults.set(1, forKey: Key.engineVersion)
    }

    private func value(for key: String, fallback: Double) -> Double {
        defaults.object(forKey: key) == nil ? fallback : defaults.double(forKey: key)
    }

    private func isLegacyDefault(_ settings: FluidBackgroundSettings) -> Bool {
        settings.pixelColor.uppercased() == "#090505"
            && settings.backgroundColor.uppercased() == "#FFD4A8"
            && abs(settings.pixelSize - 4) < 0.001
            && abs(settings.threshold - 0.29) < 0.001
            && abs(settings.speed - 0.05) < 0.001
            && abs(settings.zoom - 3.55) < 0.001
    }
}
