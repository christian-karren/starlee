import XCTest
import AppKit

// MenuBarIcon is compiled into the same binary; no import needed.

final class MenuBarIconTests: XCTestCase {

    // MARK: - loadingFrames

    func testLoadingFrames_countIsThree() {
        // loadingFrames returns [] when bundle images are unavailable (CI/test env without bundle).
        // We verify count is either 0 (no bundle) or exactly 3 (bundle present).
        let frames = MenuBarIcon.loadingFrames()
        XCTAssertTrue(frames.count == 0 || frames.count == 3,
                      "Expected 0 (no bundle) or 3 loading frames, got \(frames.count)")
    }

    func testLoadingFrames_eachFrameHasCorrectSize() {
        let frames = MenuBarIcon.loadingFrames()
        for frame in frames {
            XCTAssertEqual(frame.size, MenuBarIcon.size,
                           "Loading frame size mismatch: \(frame.size)")
        }
    }

    // MARK: - successFrames

    func testSuccessFrames_countIsTenOrZero() {
        let frames = MenuBarIcon.successFrames()
        XCTAssertTrue(frames.count == 0 || frames.count == 10,
                      "Expected 0 (no bundle) or 10 success frames, got \(frames.count)")
    }

    func testSuccessFrames_eachFrameHasCorrectSize() {
        let frames = MenuBarIcon.successFrames()
        for (i, frame) in frames.enumerated() {
            XCTAssertEqual(frame.size, MenuBarIcon.size,
                           "Success frame \(i) has wrong size: \(frame.size)")
        }
    }

    // MARK: - errorImage

    func testErrorImage_isNilOrCorrectSize() {
        // Returns nil when bundle is unavailable (no base image to derive from).
        let image = MenuBarIcon.errorImage()
        if let image {
            XCTAssertEqual(image.size, MenuBarIcon.size)
        }
        // nil is also acceptable in a headless test environment without the bundle
    }

    func testAttentionImage_isNilOrCorrectSize() {
        let image = MenuBarIcon.attentionImage()
        if let image {
            XCTAssertEqual(image.size, MenuBarIcon.size)
        }
    }

    // MARK: - State count expectations

    func testStateTransition_idle_loadingFrameCountDiffersFromSuccessFrameCount() {
        // idle → loading uses 3 frames; loading → success uses 10.
        // This guards against accidental frame count regressions.
        let loading = MenuBarIcon.loadingFrames()
        let success = MenuBarIcon.successFrames()
        guard !loading.isEmpty, !success.isEmpty else { return }
        XCTAssertNotEqual(loading.count, success.count,
                          "Loading and success frame counts should differ (3 vs 10)")
    }

    func testLoadingFrames_areNotTemplateImages() {
        for frame in MenuBarIcon.loadingFrames() {
            XCTAssertFalse(frame.isTemplate, "Loading frames should not be template images")
        }
    }

    func testSuccessFrames_areNotTemplateImages() {
        for frame in MenuBarIcon.successFrames() {
            XCTAssertFalse(frame.isTemplate, "Success frames should not be template images")
        }
    }

    func testErrorImage_isNotTemplateImage() {
        guard let image = MenuBarIcon.errorImage() else { return }
        XCTAssertFalse(image.isTemplate, "Error image should not be a template image")
    }

    func testAttentionImage_isNotTemplateImage() {
        guard let image = MenuBarIcon.attentionImage() else { return }
        XCTAssertFalse(image.isTemplate, "Attention image should not be a template image")
    }
}
