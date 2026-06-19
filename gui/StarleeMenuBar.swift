import AppKit
import Foundation
import UserNotifications

@main
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var engineProcess: Process?
    private let home = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Starlee")

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.accessory)
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let image = NSImage(systemSymbolName: "sparkle.magnifyingglass", accessibilityDescription: "Starlee") {
            image.isTemplate = true
            statusItem.button?.image = image
        } else {
            statusItem.button?.title = "★"
        }
        statusItem.button?.toolTip = "Starlee"
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
        rebuildMenu()
    }

    private func rebuildMenu() {
        let menu = NSMenu()
        let doctor = runJSON(["doctor"])
        let status = doctor?["status"] as? [String: Any]
        let count = (status?["capture_count"] as? NSNumber)?.intValue ?? 0
        let ok = (doctor?["ok"] as? Bool) ?? false
        let summary = NSMenuItem(title: "\(ok ? "●" : "●") \(count) captures · \(ok ? "ready" : "needs setup")", action: nil, keyEquivalent: "")
        summary.attributedTitle = coloredTitle(summary.title, ok ? .systemGreen : .systemOrange)
        summary.isEnabled = false
        menu.addItem(summary)
        menu.addItem(.separator())

        menu.addItem(item("Save Current Article", #selector(saveCurrentArticle), key: "s"))
        menu.addItem(.separator())
        menu.addItem(item("Search Starlee…", #selector(search)))
        menu.addItem(item("Capture pasted text…", #selector(captureText)))

        let recentItem = NSMenuItem(title: "Recent", action: nil, keyEquivalent: "")
        let recentMenu = NSMenu()
        if let recent = runJSONArray(["recent", "--limit", "8"]) {
            for value in recent {
                let title = value["title"] as? String ?? "Untitled"
                let entry = NSMenuItem(title: title, action: #selector(showRecent(_:)), keyEquivalent: "")
                entry.representedObject = value
                entry.target = self
                recentMenu.addItem(entry)
            }
        }
        if recentMenu.items.isEmpty { recentMenu.addItem(withTitle: "No captures yet", action: nil, keyEquivalent: "") }
        recentItem.submenu = recentMenu
        menu.addItem(recentItem)

        menu.addItem(.separator())
        menu.addItem(item("Browser Setup…", #selector(browserSetup)))
        menu.addItem(item("Run Setup Diagnostics…", #selector(showDoctor)))
        menu.addItem(item("Open Vault", #selector(openVault)))
        menu.addItem(item("Start Capture Endpoint", #selector(startEngine)))
        menu.addItem(item("Stop Capture Endpoint", #selector(stopEngine)))
        menu.addItem(item("Refresh", #selector(refresh)))
        menu.addItem(.separator())
        menu.addItem(item("Quit Starlee", #selector(quit)))
        statusItem.menu = menu
    }

    private func item(_ title: String, _ action: Selector, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    @objc private func saveCurrentArticle() {
        startEngine()
        guard let config = localConfig(), let token = config["capture_token"] as? String else {
            show(title: "Starlee setup needed", message: "Run Starlee setup, then reload the browser extension.")
            return
        }
        let port = (config["capture_port"] as? NSNumber)?.intValue ?? 47291
        let result = postJSON(
            url: URL(string: "http://127.0.0.1:\(port)/capture-request")!,
            token: token,
            body: ["source": "menu-bar"]
        )
        if result.ok {
            notify(title: "Starlee capture requested", body: "The browser extension will save the active article.")
        } else {
            notify(title: "Starlee capture needs attention", body: result.message)
            show(title: "Starlee capture needs attention", message: result.message)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { self.rebuildMenu() }
    }

    @objc private func search() {
        guard let query = prompt(title: "Search Starlee", label: "Question or topic") else { return }
        let output = run(["search", "--limit", "8", query])
        show(title: "Starlee Search", message: output.isEmpty ? "No results." : output)
    }

    @objc private func captureText() {
        guard let title = prompt(title: "Capture", label: "Title") else { return }
        guard let text = prompt(title: "Capture", label: "Text") else { return }
        let output = run(["capture-text", "--title", title, "--text", text])
        show(title: "Saved to Starlee", message: output)
        rebuildMenu()
    }

    @objc private func showRecent(_ sender: NSMenuItem) {
        guard let value = sender.representedObject as? [String: Any] else { return }
        show(title: value["title"] as? String ?? "Starlee", message: value["snippet"] as? String ?? "")
    }

    @objc private func openVault() {
        NSWorkspace.shared.open(home.appendingPathComponent("vault"))
    }

    @objc private func browserSetup() {
        let extensionURL = home.appendingPathComponent("sensor-extension")
        NSWorkspace.shared.activateFileViewerSelecting([extensionURL])
        if let chromeURL = URL(string: "chrome://extensions") {
            NSWorkspace.shared.open(chromeURL)
        }
        show(
            title: "Browser setup",
            message: "Load or reload the selected folder in your Chromium browser:\n\n\(extensionURL.path)\n\nSafari support will use a bundled Safari Web Extension in the next slice."
        )
    }

    @objc private func showDoctor() {
        show(title: "Starlee Diagnostics", message: run(["doctor"]))
    }

    @objc private func startEngine() {
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

    @objc private func stopEngine() {
        engineProcess?.terminate()
        engineProcess = nil
    }

    @objc private func refresh() { rebuildMenu() }
    @objc private func quit() { stopEngine(); NSApplication.shared.terminate(nil) }

    private func prompt(title: String, label: String) -> String? {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = label
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 360, height: 24))
        alert.accessoryView = field
        return alert.runModal() == .alertFirstButtonReturn ? field.stringValue : nil
    }

    private func show(title: String, message: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = String(message.prefix(5000))
        alert.runModal()
    }

    private func notify(title: String, body: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }

    private func run(_ arguments: [String]) -> String {
        let process = Process()
        let pipe = Pipe()
        process.executableURL = cliURL()
        process.arguments = ["--home", home.path] + arguments
        process.standardOutput = pipe
        process.standardError = pipe
        do { try process.run(); process.waitUntilExit() } catch { return error.localizedDescription }
        return String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
    }

    private func runJSON(_ arguments: [String]) -> [String: Any]? {
        guard let data = run(arguments).data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    private func runJSONArray(_ arguments: [String]) -> [[String: Any]]? {
        guard let data = run(arguments).data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
    }

    private func localConfig() -> [String: Any]? {
        let url = home.appendingPathComponent("config.json")
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONSerialization.jsonObject(with: data) as? [String: Any]
    }

    private func healthCheck() -> Bool {
        guard
            let config = localConfig(),
            let port = (config["capture_port"] as? NSNumber)?.intValue,
            let url = URL(string: "http://127.0.0.1:\(port)/health")
        else { return false }
        return (try? String(contentsOf: url).contains("ready")) == true
    }

    private func postJSON(url: URL, token: String, body: [String: Any]) -> (ok: Bool, message: String) {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.addValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.addValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        let semaphore = DispatchSemaphore(value: 0)
        var result: (Bool, String) = (false, "No response from Starlee.")
        URLSession.shared.dataTask(with: request) { data, response, error in
            defer { semaphore.signal() }
            if let error {
                result = (false, error.localizedDescription)
                return
            }
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            if (200..<300).contains(status) {
                result = (true, "Capture request sent.")
            } else {
                let text = data.flatMap { String(data: $0, encoding: .utf8) } ?? "HTTP \(status)"
                result = (false, text)
            }
        }.resume()
        _ = semaphore.wait(timeout: .now() + 3)
        return result
    }

    private func coloredTitle(_ title: String, _ color: NSColor) -> NSAttributedString {
        NSAttributedString(string: title, attributes: [.foregroundColor: color])
    }

    private func cliURL() -> URL {
        if let bundled = Bundle.main.url(forResource: "starlee", withExtension: nil) { return bundled }
        if let override = ProcessInfo.processInfo.environment["STARLEE_BINARY"] { return URL(fileURLWithPath: override) }
        return URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("target/release/starlee")
    }
}
