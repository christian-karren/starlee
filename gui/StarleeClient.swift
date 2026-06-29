import Foundation

struct PostResult {
    let ok: Bool
    let message: String
}

struct CaptureRequestPostResult {
    let ok: Bool
    let message: String
    let requestId: String?
    let status: String?
}

struct CaptureRequestStatusResult {
    let ok: Bool
    let status: String?
    let message: String
}

final class StarleeClient {
    private var engineProcess: Process?
    let home = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Starlee")
    let session: URLSession

    /// Injected for testing; defaults to the shared session in production.
    /// - Set `overrideConfig` to a dictionary to inject a fake config.
    /// - Set `blockDiskConfig` to true to make `localConfig()` always return nil
    ///   (simulates missing config without reading from disk).
    var overrideConfig: [String: Any]? = nil
    var blockDiskConfig: Bool = false

    init(session: URLSession = .shared) {
        self.session = session
    }

    func run(_ arguments: [String]) -> String {
        let process = Process()
        let pipe = Pipe()
        process.executableURL = cliURL()
        process.arguments = ["--home", home.path] + arguments
        process.standardOutput = pipe
        process.standardError = pipe
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return error.localizedDescription
        }
        return String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
    }

    func runJSON(_ arguments: [String]) -> [String: Any]? {
        guard let data = run(arguments).data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    func runJSONArray(_ arguments: [String]) -> [[String: Any]]? {
        guard let data = run(arguments).data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
    }

    func runAsync(_ arguments: [String], completion: @escaping (String) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            let output = self.run(arguments)
            DispatchQueue.main.async {
                completion(output)
            }
        }
    }

    func localConfig() -> [String: Any]? {
        if blockDiskConfig { return nil }
        if let override = overrideConfig { return override }
        let url = home.appendingPathComponent("config.json")
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    func healthCheck() -> Bool {
        guard
            let config = localConfig(),
            let port = (config["capture_port"] as? NSNumber)?.intValue,
            let url = URL(string: "http://127.0.0.1:\(port)/health")
        else { return false }
        return (try? String(contentsOf: url, encoding: .utf8).contains("ready")) == true
    }

    func startEngine() {
        guard engineProcess?.isRunning != true else { return }
        if healthCheck() { return }
        let process = Process()
        process.executableURL = cliURL()
        process.arguments = ["--home", home.path, "serve"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try? process.run()
        engineProcess = process
    }

    func stopEngine() {
        engineProcess?.terminate()
        engineProcess = nil
    }

    func requestCurrentArticleCapture() -> PostResult {
        startEngine()
        guard let config = localConfig(), let token = config["capture_token"] as? String else {
            return PostResult(ok: false, message: "Run Starlee setup, then reload the browser extension.")
        }
        let port = (config["capture_port"] as? NSNumber)?.intValue ?? 47291
        guard let url = URL(string: "http://127.0.0.1:\(port)/capture-request") else {
            return PostResult(ok: false, message: "Invalid local Starlee capture endpoint.")
        }
        return postJSON(url: url, token: token, body: ["source": "menu-bar"])
    }

    func requestCurrentArticleCapture(completion: @escaping (CaptureRequestPostResult) -> Void) {
        requestCapture(source: "menu-bar", completion: completion)
    }

    func requestChromeSetupCaptureTest(completion: @escaping (CaptureRequestPostResult) -> Void) {
        requestCapture(source: "desktop-setup-test", completion: completion)
    }

    private func requestCapture(source: String, completion: @escaping (CaptureRequestPostResult) -> Void) {
        startEngine()
        guard let config = localConfig(), let token = config["capture_token"] as? String else {
            completion(CaptureRequestPostResult(ok: false, message: "Run Starlee setup, then reload the browser extension.", requestId: nil, status: "setup_required"))
            return
        }
        let port = (config["capture_port"] as? NSNumber)?.intValue ?? 47291
        guard let url = URL(string: "http://127.0.0.1:\(port)/capture-request") else {
            completion(CaptureRequestPostResult(ok: false, message: "Invalid local Starlee capture endpoint.", requestId: nil, status: "setup_required"))
            return
        }
        postCaptureRequest(url: url, token: token, source: source, completion: completion)
    }

    func captureRequestStatus(id: String, completion: @escaping (CaptureRequestStatusResult) -> Void) {
        guard let config = localConfig(), let token = config["capture_token"] as? String else {
            completion(CaptureRequestStatusResult(ok: false, status: nil, message: "Run Starlee setup, then reload the browser extension."))
            return
        }
        let port = (config["capture_port"] as? NSNumber)?.intValue ?? 47291
        guard let url = URL(string: "http://127.0.0.1:\(port)/capture-request/status?id=\(id)") else {
            completion(CaptureRequestStatusResult(ok: false, status: nil, message: "Invalid local Starlee capture endpoint."))
            return
        }
        getCaptureRequestStatus(url: url, token: token, completion: completion)
    }

    private func postJSON(url: URL, token: String, body: [String: Any]) -> PostResult {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.addValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        let semaphore = DispatchSemaphore(value: 0)
        var result = PostResult(ok: false, message: "No response from Starlee.")
        session.dataTask(with: request) { data, response, error in
            defer { semaphore.signal() }
            if let error {
                result = PostResult(ok: false, message: error.localizedDescription)
                return
            }
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            if (200..<300).contains(status) {
                result = PostResult(ok: true, message: "Capture request sent.")
            } else {
                result = PostResult(ok: false, message: Self.responseMessage(data: data, status: status))
            }
        }.resume()
        _ = semaphore.wait(timeout: .now() + 3)
        return result
    }

    private func postCaptureRequest(
        url: URL,
        token: String,
        source: String,
        completion: @escaping (CaptureRequestPostResult) -> Void
    ) {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 5
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.addValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: ["source": source])

        session.dataTask(with: request) { data, response, error in
            let result: CaptureRequestPostResult
            if let error {
                result = CaptureRequestPostResult(ok: false, message: error.localizedDescription, requestId: nil, status: "service_unreachable")
            } else {
                let status = (response as? HTTPURLResponse)?.statusCode ?? 0
                let requestValue = data.flatMap { data -> [String: Any]? in
                    guard
                        let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                        let request = value["request"] as? [String: Any]
                    else { return nil }
                    return request
                }
                let requestId = requestValue?["id"] as? String
                let requestStatus = requestValue?["status"] as? String
                if (200..<300).contains(status), let requestId {
                    result = CaptureRequestPostResult(ok: true, message: "Capture request queued.", requestId: requestId, status: requestStatus)
                } else {
                    let text = requestValue?["message"] as? String
                        ?? Self.responseMessage(data: data, status: status)
                    result = CaptureRequestPostResult(ok: false, message: text, requestId: requestId, status: requestStatus)
                }
            }
            DispatchQueue.main.async {
                completion(result)
            }
        }.resume()
    }

    private func getCaptureRequestStatus(
        url: URL,
        token: String,
        completion: @escaping (CaptureRequestStatusResult) -> Void
    ) {
        var request = URLRequest(url: url)
        request.timeoutInterval = 5
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

        session.dataTask(with: request) { data, response, error in
            let result: CaptureRequestStatusResult
            if let error {
                result = CaptureRequestStatusResult(ok: false, status: nil, message: error.localizedDescription)
            } else {
                let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
                let requestStatus = data.flatMap { data -> [String: Any]? in
                    guard
                        let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
                    else { return nil }
                    return value["request"] as? [String: Any]
                }
                if (200..<300).contains(statusCode), let requestStatus {
                    result = CaptureRequestStatusResult(
                        ok: true,
                        status: requestStatus["status"] as? String,
                        message: requestStatus["message"] as? String ?? ""
                    )
                } else {
                    let text = Self.responseMessage(data: data, status: statusCode)
                    result = CaptureRequestStatusResult(ok: false, status: nil, message: text)
                }
            }
            DispatchQueue.main.async {
                completion(result)
            }
        }.resume()
    }

    private func cliURL() -> URL {
        if let bundled = Bundle.main.url(forResource: "starlee", withExtension: nil) { return bundled }
        if let override = ProcessInfo.processInfo.environment["STARLEE_BINARY"] {
            return URL(fileURLWithPath: override)
        }
        return URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("target/release/starlee")
    }

    private static func responseMessage(data: Data?, status: Int) -> String {
        guard
            let data,
            let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return data.flatMap { String(data: $0, encoding: .utf8) } ?? "HTTP \(status)"
        }
        if let message = value["message"] as? String {
            return message
        }
        if let error = value["error"] as? String {
            return error
        }
        if
            let request = value["request"] as? [String: Any],
            let message = request["message"] as? String
        {
            return message
        }
        return String(data: data, encoding: .utf8) ?? "HTTP \(status)"
    }
}
