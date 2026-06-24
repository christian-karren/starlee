import AppKit

struct FluidBackgroundSettings: Equatable {
    static let defaultPixelColor = "#062F64"
    static let defaultBackgroundColor = "#F9E4B6"

    var pixelColor: String
    var backgroundColor: String
    var pixelSize: Double
    var threshold: Double
    var speed: Double
    var zoom: Double

    static let `default` = FluidBackgroundSettings(
        pixelColor: defaultPixelColor,
        backgroundColor: defaultBackgroundColor,
        pixelSize: 6,
        threshold: 0.31,
        speed: 0.02,
        zoom: 4.8
    )

    var webPayload: [String: Any] {
        [
            "pixelColor": pixelColor,
            "backgroundColor": backgroundColor,
            "pixelSize": pixelSize,
            "threshold": threshold,
            "speed": speed,
            "zoom": zoom
        ]
    }

    var webPayloadJSON: String {
        guard
            JSONSerialization.isValidJSONObject(webPayload),
            let data = try? JSONSerialization.data(withJSONObject: webPayload, options: []),
            let json = String(data: data, encoding: .utf8)
        else {
            return ##"{"pixelColor":"#062F64","backgroundColor":"#F9E4B6","pixelSize":6,"threshold":0.31,"speed":0.02,"zoom":4.8}"##
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
    static let all: [FluidBackgroundLook] = [
        FluidBackgroundLook(name: "Navy Cream", settings: .default),
        FluidBackgroundLook(
            name: "Ink Paper",
            settings: FluidBackgroundSettings(
                pixelColor: "#111111",
                backgroundColor: "#F6F0E4",
                pixelSize: 5,
                threshold: 0.34,
                speed: 0.015,
                zoom: 5.2
            )
        ),
        FluidBackgroundLook(
            name: "Blueprint",
            settings: FluidBackgroundSettings(
                pixelColor: "#D8FBFF",
                backgroundColor: "#072A62",
                pixelSize: 5,
                threshold: 0.37,
                speed: 0.018,
                zoom: 5.8
            )
        ),
        FluidBackgroundLook(
            name: "Ember Newsprint",
            settings: FluidBackgroundSettings(
                pixelColor: "#2A1712",
                backgroundColor: "#F1C58F",
                pixelSize: 6,
                threshold: 0.36,
                speed: 0.012,
                zoom: 4.4
            )
        )
    ]
}

final class FluidBackgroundSettingsStore {
    private enum Key {
        static let pixelColor = "StarleeFluidPixelColor"
        static let backgroundColor = "StarleeFluidBackgroundColor"
        static let pixelSize = "StarleeFluidPixelSize"
        static let threshold = "StarleeFluidThreshold"
        static let speed = "StarleeFluidSpeed"
        static let zoom = "StarleeFluidZoom"
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
            zoom: value(for: Key.zoom, fallback: fallback.zoom)
        )
        if isLegacyDefault(settings) {
            save(fallback)
            return fallback
        }
        return settings
    }

    func save(_ settings: FluidBackgroundSettings) {
        defaults.set(settings.pixelColor, forKey: Key.pixelColor)
        defaults.set(settings.backgroundColor, forKey: Key.backgroundColor)
        defaults.set(settings.pixelSize, forKey: Key.pixelSize)
        defaults.set(settings.threshold, forKey: Key.threshold)
        defaults.set(settings.speed, forKey: Key.speed)
        defaults.set(settings.zoom, forKey: Key.zoom)
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
