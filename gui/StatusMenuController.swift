import AppKit

final class StatusMenuController: NSObject {
    private let statusItem: NSStatusItem
    private let client: StarleeClient
    private let notifier: NotificationController
    private var managementMenu = NSMenu()
    private var isCapturing = false
    private var loadingWorkItem: DispatchWorkItem?
    private var timeoutWorkItem: DispatchWorkItem?
    private var statusPollWorkItem: DispatchWorkItem?
    private var animationTimer: Timer?
    private var defaultImage: NSImage?

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
        managementMenu = makeManagementMenu()
        installDirectCaptureAction()
    }

    private func makeManagementMenu() -> NSMenu {
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
        return menu
    }

    private func installDirectCaptureAction() {
        statusItem.menu = nil
        guard let button = statusItem.button else { return }
        defaultImage = defaultImage ?? button.image
        button.target = self
        button.action = #selector(handleStatusItemClick(_:))
        button.sendAction(on: [.leftMouseUp])
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

    @objc private func handleStatusItemClick(_ sender: NSStatusBarButton) {
        if NSApp.currentEvent?.modifierFlags.contains(.option) == true {
            showManagementMenu(from: sender)
            return
        }
        captureFromStatusItem()
    }

    private func captureFromStatusItem() {
        guard !isCapturing else { return }
        isCapturing = true

        let loading = DispatchWorkItem { [weak self] in
            self?.startLoadingAnimation()
        }
        loadingWorkItem = loading
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8, execute: loading)

        let timeout = DispatchWorkItem { [weak self] in
            self?.finishCapture(PostResult(ok: false, message: "No response from Starlee."))
        }
        timeoutWorkItem = timeout
        DispatchQueue.main.asyncAfter(deadline: .now() + 5, execute: timeout)

        client.requestCurrentArticleCapture { [weak self] result in
            guard let self else { return }
            if result.ok, let requestId = result.requestId {
                self.pollCaptureRequestStatus(id: requestId)
            } else {
                self.finishCapture(PostResult(ok: false, message: result.message))
            }
        }
    }

    private func finishCapture(_ result: PostResult) {
        guard isCapturing else { return }
        loadingWorkItem?.cancel()
        timeoutWorkItem?.cancel()
        statusPollWorkItem?.cancel()
        loadingWorkItem = nil
        timeoutWorkItem = nil
        statusPollWorkItem = nil
        stopAnimationTimer()

        if result.ok {
            playSuccessAnimation()
        } else {
            playErrorAnimation(message: result.message)
        }
    }

    private func pollCaptureRequestStatus(id: String) {
        guard isCapturing else { return }
        client.captureRequestStatus(id: id) { [weak self] result in
            guard let self, self.isCapturing else { return }
            if !result.ok {
                self.finishCapture(PostResult(ok: false, message: result.message))
                return
            }
            switch result.status {
            case "capture_saved":
                self.finishCapture(PostResult(ok: true, message: result.message))
            case "capture_failed", "service_down", "token_missing", "token_invalid", "permission_denied", "no_active_tab", "empty_extract", "payload_too_large":
                self.finishCapture(PostResult(ok: false, message: result.message.isEmpty ? "Starlee capture failed." : result.message))
            default:
                let next = DispatchWorkItem { [weak self] in
                    self?.pollCaptureRequestStatus(id: id)
                }
                self.statusPollWorkItem = next
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.35, execute: next)
            }
        }
    }

    private func showManagementMenu(from button: NSStatusBarButton) {
        rebuildMenu()
        managementMenu.popUp(
            positioning: nil,
            at: NSPoint(x: 0, y: button.bounds.height + 3),
            in: button
        )
        installDirectCaptureAction()
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

    private func startLoadingAnimation() {
        let frames = MenuBarIcon.loadingFrames()
        guard !frames.isEmpty else { return }
        playRepeating(frames: frames, interval: 0.28)
    }

    private func playSuccessAnimation() {
        let frames = MenuBarIcon.successFrames()
        guard !frames.isEmpty else {
            resetAfterFeedback(delay: 1.2)
            return
        }
        playOnce(frames: frames, interval: 0.13) { [weak self] in
            self?.resetAfterFeedback(delay: 0)
        }
    }

    private func playErrorAnimation(message: String) {
        if let errorImage = MenuBarIcon.errorImage() {
            statusItem.button?.image = errorImage
        }
        NSLog("Starlee capture failed: \(message)")
        resetAfterFeedback(delay: 1.5)
    }

    private func playRepeating(frames: [NSImage], interval: TimeInterval) {
        stopAnimationTimer()
        var index = 0
        statusItem.button?.image = frames[index]
        animationTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            index = (index + 1) % frames.count
            self?.statusItem.button?.image = frames[index]
        }
    }

    private func playOnce(frames: [NSImage], interval: TimeInterval, completion: @escaping () -> Void) {
        stopAnimationTimer()
        var index = 0
        statusItem.button?.image = frames[index]
        animationTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] timer in
            index += 1
            guard index < frames.count else {
                timer.invalidate()
                self?.animationTimer = nil
                completion()
                return
            }
            self?.statusItem.button?.image = frames[index]
        }
    }

    private func resetAfterFeedback(delay: TimeInterval) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            self?.isCapturing = false
            self?.resetIcon()
        }
    }

    private func resetIcon() {
        statusItem.button?.image = defaultImage ?? MenuBarIcon.makeImage()
    }

    private func stopAnimationTimer() {
        animationTimer?.invalidate()
        animationTimer = nil
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
