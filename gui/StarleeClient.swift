import Foundation
import AppKit

struct PostResult {
    let ok: Bool
    let message: String
    let status: String?

    init(ok: Bool, message: String, status: String? = nil) {
        self.ok = ok
        self.message = message
        self.status = status
    }
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

struct CaptureDiagnosticPayload {
    let requestId: String
    let component: String
    let event: String
    let status: String
    let source: String
    let message: String
    let safeMetadata: [String: String]
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
    var overrideTargetBrowser: String? = nil

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
        guard let targetBrowser = targetBrowserForCapture() else {
            return PostResult(
                ok: false,
                message: "Make Chrome, Safari, or Firefox the active browser window, then capture again.",
                status: "setup_required"
            )
        }
        let port = (config["capture_port"] as? NSNumber)?.intValue ?? 47291
        guard let url = URL(string: "http://127.0.0.1:\(port)/capture-request") else {
            return PostResult(ok: false, message: "Invalid local Starlee capture endpoint.")
        }
        return postJSON(url: url, token: token, body: [
            "source": "menu-bar",
            "target_browser": targetBrowser
        ])
    }

    func requestCurrentArticleCapture(completion: @escaping (CaptureRequestPostResult) -> Void) {
        guard let targetBrowser = targetBrowserForCapture() else {
            completion(CaptureRequestPostResult(
                ok: false,
                message: "Make Chrome, Safari, or Firefox the active browser window, then capture again.",
                requestId: nil,
                status: "setup_required"
            ))
            return
        }
        requestCapture(source: "menu-bar", targetBrowser: targetBrowser, completion: completion)
    }

    func requestChromeSetupCaptureTest(completion: @escaping (CaptureRequestPostResult) -> Void) {
        requestCapture(source: "desktop-setup-test", targetBrowser: "Chrome", completion: completion)
    }

    private func requestCapture(
        source: String,
        targetBrowser: String,
        completion: @escaping (CaptureRequestPostResult) -> Void
    ) {
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
        postCaptureRequest(
            url: url,
            token: token,
            source: source,
            targetBrowser: targetBrowser,
            completion: completion
        )
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

    func recordCaptureDiagnostic(_ diagnostic: CaptureDiagnosticPayload) {
        guard
            let config = localConfig(),
            let token = config["capture_token"] as? String
        else { return }
        let port = (config["capture_port"] as? NSNumber)?.intValue ?? 47291
        guard let url = URL(string: "http://127.0.0.1:\(port)/capture-diagnostics/event") else { return }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 2
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.addValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: [
            "timestamp": ISO8601DateFormatter().string(from: Date()),
            "component": diagnostic.component,
            "event": diagnostic.event,
            "request_id": diagnostic.requestId,
            "status": diagnostic.status,
            "source": diagnostic.source,
            "message": diagnostic.message,
            "safe_metadata": diagnostic.safeMetadata
        ])
        session.dataTask(with: request).resume()
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
        targetBrowser: String,
        completion: @escaping (CaptureRequestPostResult) -> Void
    ) {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.timeoutInterval = 5
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.addValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: [
            "source": source,
            "target_browser": targetBrowser
        ])

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

    private func targetBrowserForCapture() -> String? {
        if let overrideTargetBrowser {
            return Self.normalizedBrowserName(overrideTargetBrowser)
        }
        let app = NSWorkspace.shared.frontmostApplication
        return Self.browserName(
            bundleIdentifier: app?.bundleIdentifier,
            localizedName: app?.localizedName
        )
        ?? BrowserActivityTracker.shared.lastSupportedBrowser
    }

    static func browserName(bundleIdentifier: String?, localizedName: String?) -> String? {
        let bundle = (bundleIdentifier ?? "").lowercased()
        let name = (localizedName ?? "").lowercased()
        if bundle == "com.apple.safari" || name == "safari" {
            return "Safari"
        }
        if bundle == "org.mozilla.firefox" || name.contains("firefox") {
            return "Firefox"
        }
        if bundle == "com.google.chrome" || name.contains("chrome") {
            return "Chrome"
        }
        return nil
    }

    static func normalizedBrowserName(_ value: String) -> String? {
        switch value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "chrome", "google chrome", "chromium":
            return "Chrome"
        case "safari":
            return "Safari"
        case "firefox", "mozilla firefox":
            return "Firefox"
        default:
            return nil
        }
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

final class BrowserActivityTracker {
    static let shared = BrowserActivityTracker()
    private static let supportedBrowserMemoryWindow: TimeInterval = 30

    private var lastKnownSupportedBrowser: String?
    private var lastSupportedBrowserAt: Date?
    private var observer: NSObjectProtocol?

    private init() {}

    var lastSupportedBrowser: String? {
        guard
            let browser = lastKnownSupportedBrowser,
            let activatedAt = lastSupportedBrowserAt,
            Date().timeIntervalSince(activatedAt) <= Self.supportedBrowserMemoryWindow
        else {
            return nil
        }
        return browser
    }

    func start(workspace: NSWorkspace = .shared) {
        if let browser = StarleeClient.browserName(
            bundleIdentifier: workspace.frontmostApplication?.bundleIdentifier,
            localizedName: workspace.frontmostApplication?.localizedName
        ) {
            record(browser)
        }
        guard observer == nil else { return }
        observer = workspace.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard
                let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication,
                let browser = StarleeClient.browserName(
                    bundleIdentifier: app.bundleIdentifier,
                    localizedName: app.localizedName
                )
            else { return }
            self?.record(browser)
        }
    }

    private func record(_ browser: String) {
        lastKnownSupportedBrowser = browser
        lastSupportedBrowserAt = Date()
    }
}
