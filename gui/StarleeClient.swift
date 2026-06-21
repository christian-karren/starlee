import Foundation

struct PostResult {
    let ok: Bool
    let message: String
}

final class StarleeClient {
    private var engineProcess: Process?
    let home = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Starlee")

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

    func localConfig() -> [String: Any]? {
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

    private func postJSON(url: URL, token: String, body: [String: Any]) -> PostResult {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.addValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        let semaphore = DispatchSemaphore(value: 0)
        var result = PostResult(ok: false, message: "No response from Starlee.")
        URLSession.shared.dataTask(with: request) { data, response, error in
            defer { semaphore.signal() }
            if let error {
                result = PostResult(ok: false, message: error.localizedDescription)
                return
            }
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            if (200..<300).contains(status) {
                result = PostResult(ok: true, message: "Capture request sent.")
            } else {
                let text = data.flatMap { String(data: $0, encoding: .utf8) } ?? "HTTP \(status)"
                result = PostResult(ok: false, message: text)
            }
        }.resume()
        _ = semaphore.wait(timeout: .now() + 3)
        return result
    }

    private func cliURL() -> URL {
        if let bundled = Bundle.main.url(forResource: "starlee", withExtension: nil) { return bundled }
        if let override = ProcessInfo.processInfo.environment["STARLEE_BINARY"] {
            return URL(fileURLWithPath: override)
        }
        return URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("target/release/starlee")
    }
}
