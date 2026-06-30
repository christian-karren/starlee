import XCTest
import Foundation

// All GUI source files are compiled into the same test binary — no @testable import needed.

final class StarleeClientTests: XCTestCase {

    // MARK: - Helpers

    private func makeClient(
        data: Data? = nil,
        statusCode: Int = 200,
        error: Error? = nil,
        port: Int = 47291,
        token: String = "test-token"
    ) -> (StarleeClient, MockURLSession) {
        let session = MockURLSession()
        session.stubbedData = data
        session.stubbedStatusCode = statusCode
        session.stubbedError = error
        let client = StarleeClient(session: session)
        client.overrideConfig = ["capture_port": port as NSNumber, "capture_token": token]
        client.overrideTargetBrowser = "Chrome"
        return (client, session)
    }

    // MARK: - Endpoint path

    func testRequestCapture_hitsCapturePath() {
        let responseBody = jsonData([
            "request": ["id": "req-1", "status": "queued", "message": ""] as [String: Any]
        ])
        let (client, session) = makeClient(data: responseBody)

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertTrue(result.ok, "Expected ok=true, got message: \(result.message)")
            XCTAssertEqual(result.requestId, "req-1")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        XCTAssertEqual(session.capturedRequests.count, 1)
        let req = session.capturedRequests[0]
        XCTAssertEqual(req.url?.path, "/capture-request")
        XCTAssertEqual(req.httpMethod, "POST")
        let body = try? JSONSerialization.jsonObject(with: req.httpBody ?? Data()) as? [String: Any]
        XCTAssertEqual(body?["target_browser"] as? String, "Chrome")
    }

    // MARK: - Auth header

    func testRequestCapture_sendsAuthHeader() {
        let token = "super-secret"
        let responseBody = jsonData([
            "request": ["id": "req-2", "status": "queued", "message": ""] as [String: Any]
        ])
        let (client, session) = makeClient(data: responseBody, token: token)

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { _ in exp.fulfill() }
        wait(for: [exp], timeout: 2)

        let authHeader = session.capturedRequests[0].value(forHTTPHeaderField: "Authorization")
        XCTAssertEqual(authHeader, "Bearer \(token)")
    }

    // MARK: - Content-Type header

    func testRequestCapture_sendsContentTypeJSON() {
        let responseBody = jsonData([
            "request": ["id": "r", "status": "queued", "message": ""] as [String: Any]
        ])
        let (client, session) = makeClient(data: responseBody)

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { _ in exp.fulfill() }
        wait(for: [exp], timeout: 2)

        let ct = session.capturedRequests[0].value(forHTTPHeaderField: "Content-Type")
        XCTAssertEqual(ct, "application/json")
    }

    // MARK: - Network error

    func testRequestCapture_networkError_returnsFalse() {
        let (client, _) = makeClient(error: URLError(.notConnectedToInternet))

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertFalse(result.ok)
            XCTAssertNil(result.requestId)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)
    }

    // MARK: - Non-2xx response

    func testRequestCapture_401_returnsFalse() {
        let responseBody = jsonData(["message": "unauthorized"])
        let (client, _) = makeClient(data: responseBody, statusCode: 401)

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertFalse(result.ok)
            XCTAssertFalse(result.message.isEmpty)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)
    }

    func testRequestCapture_setupRejectionPreservesRequestStatusForDiagnostics() {
        let responseBody = jsonData([
            "request": [
                "id": "req-setup",
                "status": "extension_unavailable",
                "message": "Load or reload the Starlee browser extension, then try again."
            ] as [String: Any]
        ])
        let (client, _) = makeClient(data: responseBody, statusCode: 409)

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertFalse(result.ok)
            XCTAssertEqual(result.requestId, "req-setup")
            XCTAssertEqual(result.status, "extension_unavailable")
            XCTAssertEqual(result.message, "Load or reload the Starlee browser extension, then try again.")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)
    }

    // MARK: - Status endpoint path

    func testCaptureRequestStatus_hitsStatusPath() {
        let id = "abc-123"
        let responseBody = jsonData([
            "request": ["status": "capture_saved", "message": "done"] as [String: Any]
        ])
        let (client, session) = makeClient(data: responseBody)

        let exp = expectation(description: "done")
        client.captureRequestStatus(id: id) { result in
            XCTAssertTrue(result.ok)
            XCTAssertEqual(result.status, "capture_saved")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        let req = session.capturedRequests[0]
        XCTAssertEqual(req.url?.path, "/capture-request/status")
        XCTAssertTrue(req.url?.query?.contains("id=\(id)") == true)
    }

    // MARK: - Status auth header

    func testCaptureRequestStatus_sendsAuthHeader() {
        let token = "status-secret"
        let responseBody = jsonData([
            "request": ["status": "queued", "message": ""] as [String: Any]
        ])
        let (client, session) = makeClient(data: responseBody, token: token)

        let exp = expectation(description: "done")
        client.captureRequestStatus(id: "x") { _ in exp.fulfill() }
        wait(for: [exp], timeout: 2)

        let authHeader = session.capturedRequests[0].value(forHTTPHeaderField: "Authorization")
        XCTAssertEqual(authHeader, "Bearer \(token)")
    }

    // MARK: - Missing config: no network request made

    func testRequestCapture_missingConfig_noNetworkRequest() {
        let session = MockURLSession()
        let client = StarleeClient(session: session)
        client.blockDiskConfig = true
        client.overrideTargetBrowser = "Chrome"

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertFalse(result.ok)
            XCTAssertNil(result.requestId)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        XCTAssertEqual(session.capturedRequests.count, 0,
                       "No network request should be made without config")
    }

    func testCaptureRequestStatus_missingConfig_noNetworkRequest() {
        let session = MockURLSession()
        let client = StarleeClient(session: session)
        client.blockDiskConfig = true

        let exp = expectation(description: "done")
        client.captureRequestStatus(id: "x") { result in
            XCTAssertFalse(result.ok)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        XCTAssertEqual(session.capturedRequests.count, 0)
    }

    // MARK: - captureRequestStatus error response

    func testCaptureRequestStatus_networkError_returnsFalse() {
        let (client, _) = makeClient(error: URLError(.timedOut))

        let exp = expectation(description: "done")
        client.captureRequestStatus(id: "y") { result in
            XCTAssertFalse(result.ok)
            XCTAssertNil(result.status)
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)
    }

    // MARK: - Chrome setup capture test

    func testRequestChromeSetupCaptureTest_sendsCorrectSource() {
        let responseBody = jsonData([
            "request": ["id": "chrome-req", "status": "queued", "message": ""] as [String: Any]
        ])
        let (client, session) = makeClient(data: responseBody)

        let exp = expectation(description: "done")
        client.requestChromeSetupCaptureTest { result in
            XCTAssertTrue(result.ok)
            XCTAssertEqual(result.requestId, "chrome-req")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        XCTAssertEqual(session.capturedRequests.count, 1)
        XCTAssertEqual(session.capturedRequests[0].url?.path, "/capture-request")

        // Verify the body contains source = "desktop-setup-test"
        if let body = session.capturedRequests[0].httpBody,
           let json = try? JSONSerialization.jsonObject(with: body) as? [String: Any] {
            XCTAssertEqual(json["source"] as? String, "desktop-setup-test")
            XCTAssertEqual(json["target_browser"] as? String, "Chrome")
        } else {
            XCTFail("Request body should be valid JSON with 'source' key")
        }
    }

    func testRequestCapture_safariUnsupportedDoesNotEnqueueRequest() {
        let session = MockURLSession()
        let client = StarleeClient(session: session)
        client.overrideConfig = ["capture_port": 47291 as NSNumber, "capture_token": "tok"]
        client.overrideTargetBrowser = "Safari"

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertFalse(result.ok)
            XCTAssertNil(result.requestId)
            XCTAssertEqual(result.status, "setup_required")
            XCTAssertEqual(result.message, "Safari capture is not enabled in this build. Use Chrome or Firefox.")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        XCTAssertEqual(session.capturedRequests.count, 0)
    }

    func testRequestCapture_unknownTargetDoesNotEnqueueRequest() {
        let session = MockURLSession()
        let client = StarleeClient(session: session)
        client.overrideConfig = ["capture_port": 47291 as NSNumber, "capture_token": "tok"]
        client.overrideTargetBrowser = "Finder"

        let exp = expectation(description: "done")
        client.requestCurrentArticleCapture { result in
            XCTAssertFalse(result.ok)
            XCTAssertNil(result.requestId)
            XCTAssertEqual(result.status, "setup_required")
            XCTAssertEqual(result.message, "Open Chrome or Firefox to an article or YouTube page, then try again.")
            exp.fulfill()
        }
        wait(for: [exp], timeout: 2)

        XCTAssertEqual(session.capturedRequests.count, 0)
    }

    func testBrowserNameMappingRecognizesSupportedBrowsersOnly() {
        XCTAssertEqual(StarleeClient.browserName(bundleIdentifier: "com.apple.Safari", localizedName: "Safari"), "Safari")
        XCTAssertEqual(StarleeClient.browserName(bundleIdentifier: "org.mozilla.firefox", localizedName: "Firefox"), "Firefox")
        XCTAssertEqual(StarleeClient.browserName(bundleIdentifier: "com.google.Chrome", localizedName: "Google Chrome"), "Chrome")
        XCTAssertNil(StarleeClient.browserName(bundleIdentifier: "com.apple.finder", localizedName: "Finder"))
    }

    func testCaptureTraceSummaryIncludesRoutingAndNextAction() {
        let raw = """
        {
          "result_code": "capture_saved",
          "user_safe_message": "Saved to Starlee.",
          "next_action": "Open another page.",
          "browser": "Safari",
          "request_status": {
            "requested_browser": "Safari",
            "handling_browser": "Safari"
          },
          "events": [
            {"safe_metadata": {"page_type": "youtube"}}
          ]
        }
        """

        let summary = StatusMenuController.captureTraceSummary(rawJSON: raw)

        XCTAssertTrue(summary.contains("Requested browser: Safari"))
        XCTAssertTrue(summary.contains("Handling browser: Safari"))
        XCTAssertTrue(summary.contains("Page type: youtube"))
        XCTAssertTrue(summary.contains("Result: capture_saved"))
        XCTAssertTrue(summary.contains("Next action: Open another page."))
    }
}
