import Foundation

// MARK: - Mock URLSession

final class MockURLSession: URLSession, @unchecked Sendable {
    var stubbedData: Data?
    var stubbedStatusCode: Int = 200
    var stubbedError: Error?
    var capturedRequests: [URLRequest] = []
    /// When true, the completion handler is never called (simulates a hung request).
    var neverComplete: Bool = false

    override func dataTask(
        with request: URLRequest,
        completionHandler: @escaping (Data?, URLResponse?, Error?) -> Void
    ) -> URLSessionDataTask {
        capturedRequests.append(request)
        if neverComplete {
            return MockDataTask {}
        }
        let data = stubbedData
        let error = stubbedError
        let statusCode = stubbedStatusCode
        let url = request.url ?? URL(string: "http://127.0.0.1")!
        let response: HTTPURLResponse? = error == nil
            ? HTTPURLResponse(url: url, statusCode: statusCode, httpVersion: nil, headerFields: nil)
            : nil
        return MockDataTask {
            completionHandler(data, response, error)
        }
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
