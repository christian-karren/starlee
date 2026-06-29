import Foundation

// MARK: - Mock URLSession

final class MockURLSession: URLSession, @unchecked Sendable {
    struct MockResponse {
        var data: Data?
        var statusCode: Int
        var error: Error?
        var neverComplete: Bool
    }

    var stubbedData: Data?
    var stubbedStatusCode: Int = 200
    var stubbedError: Error?
    var queuedResponses: [MockResponse] = []
    var capturedRequests: [URLRequest] = []
    var onRequest: ((URLRequest) -> Void)?
    /// When true, the completion handler is never called (simulates a hung request).
    var neverComplete: Bool = false

    func enqueueResponse(
        data: Data? = nil,
        statusCode: Int = 200,
        error: Error? = nil,
        neverComplete: Bool = false
    ) {
        queuedResponses.append(MockResponse(
            data: data,
            statusCode: statusCode,
            error: error,
            neverComplete: neverComplete
        ))
    }

    override func dataTask(
        with request: URLRequest,
        completionHandler: @escaping (Data?, URLResponse?, Error?) -> Void
    ) -> URLSessionDataTask {
        capturedRequests.append(request)
        onRequest?(request)
        let responseConfig = queuedResponses.isEmpty
            ? MockResponse(data: stubbedData, statusCode: stubbedStatusCode, error: stubbedError, neverComplete: neverComplete)
            : queuedResponses.removeFirst()
        if responseConfig.neverComplete {
            return MockDataTask {}
        }
        let data = responseConfig.data
        let error = responseConfig.error
        let statusCode = responseConfig.statusCode
        let url = request.url ?? URL(string: "http://127.0.0.1")!
        let response: HTTPURLResponse? = error == nil
            ? HTTPURLResponse(url: url, statusCode: statusCode, httpVersion: nil, headerFields: nil)
            : nil
        return MockDataTask {
            completionHandler(data, response, error)
        }
    }

    override func dataTask(with request: URLRequest) -> URLSessionDataTask {
        capturedRequests.append(request)
        onRequest?(request)
        if !queuedResponses.isEmpty {
            _ = queuedResponses.removeFirst()
        }
        return MockDataTask {}
    }
}

final class MockDataTask: URLSessionDataTask, @unchecked Sendable {
    private let closure: () -> Void
    // Suppress deprecation: URLSessionDataTask.init() is the only way to subclass without Xcode project
    @available(macOS, deprecated: 10.15)
    init(_ closure: @escaping () -> Void) {
        self.closure = closure
    }
    override func resume() { closure() }
    override func cancel() {}
}

// MARK: - Helpers

func jsonData(_ dict: [String: Any]) -> Data {
    try! JSONSerialization.data(withJSONObject: dict)
}
