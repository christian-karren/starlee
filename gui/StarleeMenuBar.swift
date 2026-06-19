import AppKit
import Foundation

@main
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var engineProcess: Process?
    private let home = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Starlee")

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "★ Starlee"
        rebuildMenu()
    }

    private func rebuildMenu() {
        let menu = NSMenu()
        let status = runJSON(["status"])
        let count = (status?["capture_count"] as? NSNumber)?.intValue ?? 0
        let summary = NSMenuItem(title: "\(count) captures · local", action: nil, keyEquivalent: "")
        summary.isEnabled = false
        menu.addItem(summary)
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
        menu.addItem(item("Open Vault", #selector(openVault)))
        menu.addItem(item("Start Capture Endpoint", #selector(startEngine)))
        menu.addItem(item("Stop Capture Endpoint", #selector(stopEngine)))
        menu.addItem(item("Refresh", #selector(refresh)))
        menu.addItem(.separator())
        menu.addItem(item("Quit Starlee", #selector(quit)))
        statusItem.menu = menu
    }

    private func item(_ title: String, _ action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        return item
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

    @objc private func startEngine() {
        guard engineProcess?.isRunning != true else { return }
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

    private func cliURL() -> URL {
        if let bundled = Bundle.main.url(forResource: "starlee", withExtension: nil) { return bundled }
        if let override = ProcessInfo.processInfo.environment["STARLEE_BINARY"] { return URL(fileURLWithPath: override) }
        return URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("target/release/starlee")
    }
}
