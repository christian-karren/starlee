import AppKit

final class StatusMenuController: NSObject {
    private let statusItem: NSStatusItem
    private let client: StarleeClient
    private let notifier: NotificationController

    init(
        statusItem: NSStatusItem,
        client: StarleeClient,
        notifier: NotificationController
    ) {
        self.statusItem = statusItem
        self.client = client
        self.notifier = notifier
    }

    func rebuildMenu() {
        let menu = NSMenu()
        addSummary(to: menu)
        menu.addItem(.separator())
        menu.addItem(item("Save Current Article", #selector(saveCurrentArticle), key: "s"))
        menu.addItem(.separator())
        menu.addItem(item("Search Starlee…", #selector(search)))
        menu.addItem(item("Capture pasted text…", #selector(captureText)))
        addRecentMenu(to: menu)
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

    private func addSummary(to menu: NSMenu) {
        let doctor = client.runJSON(["doctor"])
        let status = doctor?["status"] as? [String: Any]
        let count = (status?["capture_count"] as? NSNumber)?.intValue ?? 0
        let ok = (doctor?["ok"] as? Bool) ?? false
        let title = "● \(count) captures · \(ok ? "ready" : "needs setup")"
        let summary = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        summary.attributedTitle = NSAttributedString(
            string: title,
            attributes: [.foregroundColor: ok ? NSColor.systemGreen : NSColor.systemOrange]
        )
        summary.isEnabled = false
        menu.addItem(summary)
    }

    private func addRecentMenu(to menu: NSMenu) {
        let recentItem = NSMenuItem(title: "Recent", action: nil, keyEquivalent: "")
        let recentMenu = NSMenu()
        if let recent = client.runJSONArray(["recent", "--limit", "8"]) {
            for value in recent {
                let title = value["title"] as? String ?? "Untitled"
                let entry = NSMenuItem(title: title, action: #selector(showRecent(_:)), keyEquivalent: "")
                entry.representedObject = value
                entry.target = self
                recentMenu.addItem(entry)
            }
        }
        if recentMenu.items.isEmpty {
            recentMenu.addItem(withTitle: "No captures yet", action: nil, keyEquivalent: "")
        }
        recentItem.submenu = recentMenu
        menu.addItem(recentItem)
    }

    private func item(_ title: String, _ action: Selector, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    @objc func saveCurrentArticle() {
        let result = client.requestCurrentArticleCapture()
        if result.ok {
            notifier.notify(title: "Starlee capture requested", body: "The browser extension will save the active article.")
        } else {
            notifier.notify(title: "Starlee capture needs attention", body: result.message)
            DialogPresenter.show(title: "Starlee capture needs attention", message: result.message)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { self.rebuildMenu() }
    }

    @objc private func search() {
        guard let query = DialogPresenter.prompt(title: "Search Starlee", label: "Question or topic") else { return }
        let output = client.run(["search", "--limit", "8", query])
        DialogPresenter.show(title: "Starlee Search", message: output.isEmpty ? "No results." : output)
    }

    @objc private func captureText() {
        guard let title = DialogPresenter.prompt(title: "Capture", label: "Title") else { return }
        guard let text = DialogPresenter.prompt(title: "Capture", label: "Text") else { return }
        let output = client.run(["capture-text", "--title", title, "--text", text])
        DialogPresenter.show(title: "Saved to Starlee", message: output)
        rebuildMenu()
    }

    @objc private func showRecent(_ sender: NSMenuItem) {
        guard let value = sender.representedObject as? [String: Any] else { return }
        DialogPresenter.show(title: value["title"] as? String ?? "Starlee", message: value["snippet"] as? String ?? "")
    }

    @objc private func browserSetup() {
        let extensionURL = client.home.appendingPathComponent("sensor-extension")
        NSWorkspace.shared.activateFileViewerSelecting([extensionURL])
        if let chromeURL = URL(string: "chrome://extensions") {
            NSWorkspace.shared.open(chromeURL)
        }
        DialogPresenter.show(
            title: "Browser setup",
            message: """
            Safari:
            Enable Starlee Capture in Safari Settings > Extensions, then allow it on the sites you want to save.

            Chromium:
            Load or reload the selected folder in chrome://extensions:

            \(extensionURL.path)
            """
        )
    }

    @objc private func showDoctor() {
        DialogPresenter.show(title: "Starlee Diagnostics", message: client.run(["doctor"]))
    }

    @objc private func openVault() {
        NSWorkspace.shared.open(client.home.appendingPathComponent("vault"))
    }

    @objc private func startEngine() {
        client.startEngine()
    }

    @objc private func stopEngine() {
        client.stopEngine()
    }

    @objc private func refresh() {
        rebuildMenu()
    }

    @objc private func quit() {
        client.stopEngine()
        NSApplication.shared.terminate(nil)
    }
}
