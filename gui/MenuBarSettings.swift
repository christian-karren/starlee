import Foundation

struct MenuBarSettings: Equatable {
    var favorites: Bool
    var topics: Bool
    var time: Bool
    var sources: Bool
    var companies: Bool
    var countries: Bool
    var customEntities: Bool
    var showEmptySections: Bool

    static let `default` = MenuBarSettings(
        favorites: true,
        topics: true,
        time: true,
        sources: true,
        companies: false,
        countries: false,
        customEntities: false,
        showEmptySections: false
    )

    init(
        favorites: Bool,
        topics: Bool,
        time: Bool,
        sources: Bool,
        companies: Bool,
        countries: Bool,
        customEntities: Bool,
        showEmptySections: Bool
    ) {
        self.favorites = favorites
        self.topics = topics
        self.time = time
        self.sources = sources
        self.companies = companies
        self.countries = countries
        self.customEntities = customEntities
        self.showEmptySections = showEmptySections
    }

    init(payload: [String: Any]) {
        let fallback = Self.default
        favorites = Self.bool(payload["favorites"], fallback: fallback.favorites)
        topics = Self.bool(payload["topics"], fallback: fallback.topics)
        time = Self.bool(payload["time"], fallback: fallback.time)
        sources = Self.bool(payload["sources"], fallback: fallback.sources)
        companies = Self.bool(payload["companies"], fallback: fallback.companies)
        countries = Self.bool(payload["countries"], fallback: fallback.countries)
        customEntities = Self.bool(payload["customEntities"], fallback: fallback.customEntities)
        showEmptySections = Self.bool(payload["showEmptySections"], fallback: fallback.showEmptySections)
    }

    var webPayload: [String: Any] {
        [
            "favorites": favorites,
            "topics": topics,
            "time": time,
            "sources": sources,
            "companies": companies,
            "countries": countries,
            "customEntities": customEntities,
            "showEmptySections": showEmptySections
        ]
    }

    private static func bool(_ value: Any?, fallback: Bool) -> Bool {
        if let value = value as? Bool { return value }
        if let value = value as? NSNumber { return value.boolValue }
        return fallback
    }
}

final class MenuBarSettingsStore {
    private enum Key {
        static let favorites = "StarleeMenuBarFavorites"
        static let topics = "StarleeMenuBarTopics"
        static let time = "StarleeMenuBarTime"
        static let sources = "StarleeMenuBarSources"
        static let companies = "StarleeMenuBarCompanies"
        static let countries = "StarleeMenuBarCountries"
        static let customEntities = "StarleeMenuBarCustomEntities"
        static let showEmptySections = "StarleeMenuBarShowEmptySections"
        static let hasSaved = "StarleeMenuBarHasSavedSettings"
    }

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    func load() -> MenuBarSettings {
        let fallback = MenuBarSettings.default
        guard defaults.bool(forKey: Key.hasSaved) else {
            return fallback
        }
        return MenuBarSettings(
            favorites: value(for: Key.favorites, fallback: fallback.favorites),
            topics: value(for: Key.topics, fallback: fallback.topics),
            time: value(for: Key.time, fallback: fallback.time),
            sources: value(for: Key.sources, fallback: fallback.sources),
            companies: value(for: Key.companies, fallback: fallback.companies),
            countries: value(for: Key.countries, fallback: fallback.countries),
            customEntities: value(for: Key.customEntities, fallback: fallback.customEntities),
            showEmptySections: value(for: Key.showEmptySections, fallback: fallback.showEmptySections)
        )
    }

    func save(_ settings: MenuBarSettings) {
        defaults.set(settings.favorites, forKey: Key.favorites)
        defaults.set(settings.topics, forKey: Key.topics)
        defaults.set(settings.time, forKey: Key.time)
        defaults.set(settings.sources, forKey: Key.sources)
        defaults.set(settings.companies, forKey: Key.companies)
        defaults.set(settings.countries, forKey: Key.countries)
        defaults.set(settings.customEntities, forKey: Key.customEntities)
        defaults.set(settings.showEmptySections, forKey: Key.showEmptySections)
        defaults.set(true, forKey: Key.hasSaved)
    }

    func reset() {
        save(.default)
    }

    private func value(for key: String, fallback: Bool) -> Bool {
        defaults.object(forKey: key) == nil ? fallback : defaults.bool(forKey: key)
    }
}
