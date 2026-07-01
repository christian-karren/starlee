import XCTest

final class MenuBarSettingsTests: XCTestCase {
    private var suiteName: String!
    private var defaults: UserDefaults!

    override func setUp() {
        super.setUp()
        suiteName = "StarleeMenuBarSettingsTests-\(UUID().uuidString)"
        defaults = UserDefaults(suiteName: suiteName)
        defaults.removePersistentDomain(forName: suiteName)
    }

    override func tearDown() {
        defaults.removePersistentDomain(forName: suiteName)
        defaults = nil
        suiteName = nil
        super.tearDown()
    }

    func testDefaultSettingsMatchProductDefaults() {
        let settings = MenuBarSettingsStore(defaults: defaults).load()
        XCTAssertTrue(settings.favorites)
        XCTAssertTrue(settings.topics)
        XCTAssertTrue(settings.time)
        XCTAssertTrue(settings.sources)
        XCTAssertFalse(settings.companies)
        XCTAssertFalse(settings.countries)
        XCTAssertFalse(settings.customEntities)
        XCTAssertFalse(settings.showEmptySections)
    }

    func testSavedSettingsPersist() {
        let store = MenuBarSettingsStore(defaults: defaults)
        let saved = MenuBarSettings(
            favorites: false,
            topics: true,
            time: false,
            sources: true,
            companies: true,
            countries: true,
            customEntities: true,
            showEmptySections: true
        )
        store.save(saved)
        XCTAssertEqual(store.load(), saved)
    }

    func testResetRestoresDefaults() {
        let store = MenuBarSettingsStore(defaults: defaults)
        store.save(MenuBarSettings(
            favorites: false,
            topics: false,
            time: false,
            sources: false,
            companies: true,
            countries: true,
            customEntities: true,
            showEmptySections: true
        ))
        store.reset()
        XCTAssertEqual(store.load(), .default)
    }

    func testPayloadFallsBackForMissingValues() {
        let settings = MenuBarSettings(payload: ["companies": true])
        XCTAssertTrue(settings.favorites)
        XCTAssertTrue(settings.topics)
        XCTAssertTrue(settings.time)
        XCTAssertTrue(settings.sources)
        XCTAssertTrue(settings.companies)
        XCTAssertFalse(settings.countries)
        XCTAssertFalse(settings.customEntities)
        XCTAssertFalse(settings.showEmptySections)
    }
}
