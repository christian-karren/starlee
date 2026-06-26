import XCTest

// MenuBarIcon is compiled into the same binary; no import needed.

final class MenuBarIconTests: XCTestCase {

    // MARK: - Pure constants (no AppKit rendering, safe in headless CI)

    func testSize_isExpectedDimensions() {
        XCTAssertEqual(MenuBarIcon.size.width, 22)
        XCTAssertEqual(MenuBarIcon.size.height, 22)
    }

    func testLoadingFrameCount_isThree() {
        XCTAssertEqual(MenuBarIcon.loadingFrameCount, 3)
    }

    func testSuccessFrameCount_isTen() {
        XCTAssertEqual(MenuBarIcon.successFrameCount, 10)
    }

    func testLoadingFrameCountDiffersFromSuccessFrameCount() {
        XCTAssertNotEqual(MenuBarIcon.loadingFrameCount, MenuBarIcon.successFrameCount)
    }

    // MARK: - Pulse progress math (pure, no AppKit)

    func testPulseProgress_firstFrameIsZero() {
        XCTAssertEqual(MenuBarIcon.pulseProgress(forIndex: 0), 0.0, accuracy: 0.001)
    }

    func testPulseProgress_lastFrameIsOne() {
        let last = MenuBarIcon.successFrameCount - 1
        XCTAssertEqual(MenuBarIcon.pulseProgress(forIndex: last), 1.0, accuracy: 0.001)
    }

    func testPulseProgress_midpoint() {
        // Frame 4 of [0..<10]: 4/9 ≈ 0.444
        XCTAssertEqual(MenuBarIcon.pulseProgress(forIndex: 4), 4.0 / 9.0, accuracy: 0.001)
    }

    func testPulseProgress_isMonotonicallyIncreasing() {
        let values = (0..<MenuBarIcon.successFrameCount).map { MenuBarIcon.pulseProgress(forIndex: $0) }
        for i in 1..<values.count {
            XCTAssertGreaterThan(values[i], values[i - 1],
                                 "pulseProgress must increase at each frame index")
        }
    }
}
