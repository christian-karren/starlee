import AppKit

final class StatusMenuController: NSObject {
    private static let captureTimeout: TimeInterval = 180
    static let actionableCaptureStatuses: Set<String> = [
        "permission_denied",
        "unsupported_page",
        "extension_unavailable",
        "content_script_unreachable",
        "timed_out",
        "token_missing",
        "token_invalid",
        "service_down",
        "setup_required",
        "service_unreachable"
    ]

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
    private var activeCaptureRequestId: String?

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
        menu.addItem(item("Test Chrome Capture", #selector(testChromeCapture)))
        menu.addItem(item("Run Setup Diagnostics…", #selector(showDoctor)))
        let traceItem = item("Show Last Capture Trace…", #selector(showLastCaptureTrace))
        traceItem.isEnabled = hasLastCaptureTrace()
        menu.addItem(traceItem)
        menu.addItem(item("Open Vault", #selector(openVault)))
        menu.addItem(item("Start Capture Endpoint", #selector(startEngine)))
        menu.addItem(item("Stop Capture Endpoint", #selector(stopEngine)))
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
        let bridge = status?["bridge_health"] as? [String: Any]
        let chromeSetup = bridge?["browser_setup"] as? [String: Any] ?? bridge?["chrome_setup"] as? [String: Any]
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
        let bridgeDetail = chromeSetup?["next_action"] as? String ?? bridge?["recommended_next_action"] as? String
        if let action = bridgeDetail, !action.isEmpty {
            let state = chromeSetup?["state"] as? String ?? "bridge"
            let bridgeItem = NSMenuItem(title: "Chrome \(state): \(action)", action: nil, keyEquivalent: "")
            bridgeItem.isEnabled = false
            menu.addItem(bridgeItem)
        }
        if
            let trace = client.runJSON(["diagnostics", "--last-capture"]),
            let terminal = trace["terminal_status"] as? String,
            terminal != "capture_saved"
        {
            let action = trace["recommended_next_action"] as? String ?? "Run the last capture trace."
            let failureItem = NSMenuItem(title: "Last capture: \(terminal) · \(action)", action: nil, keyEquivalent: "")
            failureItem.isEnabled = false
            menu.addItem(failureItem)
        }
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
            self?.finishCaptureNeedsAttention(message: "The browser did not finish capture in time. Reload the extension or page, then try again.")
        }
        timeoutWorkItem = timeout
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.captureTimeout, execute: timeout)

        client.requestCurrentArticleCapture { [weak self] result in
            guard let self else { return }
            if result.ok, let requestId = result.requestId {
                self.activeCaptureRequestId = requestId
                self.pollCaptureRequestStatus(id: requestId)
            } else if Self.isSetupStatus(result.status) {
                self.finishCaptureNeedsAttention(message: result.message)
            } else {
                self.finishCapture(PostResult(ok: false, message: result.message, status: result.status))
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
            recordMenuBarResult(status: "capture_saved", message: result.message, animation: "success")
            playSuccessAnimation()
        } else if Self.isSetupStatus(result.status) {
            playAttentionAnimation(message: result.message)
        } else {
            recordMenuBarResult(status: "capture_failed", message: result.message, animation: "error")
            playErrorAnimation(message: result.message)
        }
    }

    private func finishCaptureNeedsAttention(message: String) {
        guard isCapturing else { return }
        loadingWorkItem?.cancel()
        timeoutWorkItem?.cancel()
        statusPollWorkItem?.cancel()
        loadingWorkItem = nil
        timeoutWorkItem = nil
        statusPollWorkItem = nil
        stopAnimationTimer()
        playAttentionAnimation(message: message)
    }

    static func isSetupStatus(_ status: String?) -> Bool {
        guard let status else { return false }
        return actionableCaptureStatuses.contains(status)
    }

    static func isActionableCaptureStatus(_ status: String?) -> Bool {
        isSetupStatus(status)
    }

    private func pollCaptureRequestStatus(id: String) {
        guard isCapturing else { return }
        client.captureRequestStatus(id: id) { [weak self] result in
            guard let self, self.isCapturing else { return }
            if !result.ok {
                self.scheduleCaptureStatusPoll(id: id, delay: 1.0)
                return
            }
            switch result.status {
            case "capture_saved":
                self.finishCapture(PostResult(ok: true, message: result.message))
            case "permission_denied", "unsupported_page", "extension_unavailable", "content_script_unreachable", "timed_out", "setup_required", "service_down", "service_unreachable", "token_missing", "token_invalid":
                self.finishCaptureNeedsAttention(message: result.message.isEmpty ? "Starlee capture needs setup." : result.message)
            case "capture_failed":
                self.finishCapture(PostResult(ok: false, message: result.message.isEmpty ? "Starlee capture failed." : result.message))
            default:
                self.scheduleCaptureStatusPoll(id: id, delay: 0.35)
            }
        }
    }

    private func scheduleCaptureStatusPoll(id: String, delay: TimeInterval) {
        let next = DispatchWorkItem { [weak self] in
            self?.pollCaptureRequestStatus(id: id)
        }
        statusPollWorkItem = next
        DispatchQueue.main.asyncAfter(deadline: .now() + delay, execute: next)
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

    private func playAttentionAnimation(message: String) {
        if let attentionImage = MenuBarIcon.attentionImage() {
            statusItem.button?.image = attentionImage
        }
        let body = message.isEmpty ? "Open Starlee diagnostics for the next step." : message
        recordMenuBarResult(status: "needs_attention", message: body, animation: "attention")
        notifier.notify(title: "Starlee capture needs attention", body: body)
        NSLog("Starlee capture needs attention: \(body)")
        resetAfterFeedback(delay: 1.5)
    }

    private func recordMenuBarResult(status: String, message: String, animation: String) {
        guard let requestId = activeCaptureRequestId else { return }
        client.recordCaptureDiagnostic(CaptureDiagnosticPayload(
            requestId: requestId,
            component: "menu_bar",
            event: "menu_bar_capture_result_displayed",
            status: status,
            source: "menu-bar",
            message: message,
            safeMetadata: ["animation": animation]
        ))
        activeCaptureRequestId = nil
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

    @objc func browserSetup() {
        let doctor = client.runJSON(["doctor"])
        let status = doctor?["status"] as? [String: Any]
        let bridge = status?["bridge_health"] as? [String: Any] ?? [:]
        let setup = bridge["browser_setup"] as? [String: Any] ?? bridge["chrome_setup"] as? [String: Any] ?? [:]
        let extensionURL = client.home.appendingPathComponent("sensor-extension")
        NSWorkspace.shared.activateFileViewerSelecting([extensionURL])
        if let chromeURL = URL(string: "chrome://extensions") {
            NSWorkspace.shared.open(chromeURL)
        }
        let installed = (setup["installed"] as? Bool) == true ? "yes" : "no"
        let checkedIn = (setup["checked_in_recently"] as? Bool) == true ? "yes" : "no"
        let permissionNeeded = (setup["permission_needed"] as? Bool) == true ? "yes" : "no"
        let captureTest = (setup["capture_test_passed"] as? Bool) == true ? "yes" : "no"
        DialogPresenter.show(
            title: "Browser setup",
            message: """
            Browser extension:
            Load or reload the selected folder in your browser extension developer settings:

            \(extensionURL.path)

            Installed: \(installed)
            Checked in recently: \(checkedIn)
            Permission needed: \(permissionNeeded)
            Capture test passed: \(captureTest)

            State: \(setup["state"] as? String ?? "unknown")
            Detail: \(setup["detail"] as? String ?? "unknown")
            Next: \(setup["next_action"] as? String ?? bridge["recommended_next_action"] as? String ?? "Reload the extension, then run the capture test.")

            Safari:
            Enable Starlee Capture in Safari Settings > Extensions, then allow it on the sites you want to save.
            """
        )
    }

    @objc func testChromeCapture() {
        guard !isCapturing else { return }
        isCapturing = true
        startLoadingAnimation()

        let timeout = DispatchWorkItem { [weak self] in
            self?.finishCaptureNeedsAttention(message: "Chrome capture test timed out. Reload the extension or page, then try again.")
        }
        timeoutWorkItem = timeout
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.captureTimeout, execute: timeout)

        client.requestChromeSetupCaptureTest { [weak self] result in
            guard let self else { return }
            if result.ok, let requestId = result.requestId {
                self.activeCaptureRequestId = requestId
                self.pollCaptureRequestStatus(id: requestId)
            } else if Self.isSetupStatus(result.status) {
                self.finishCaptureNeedsAttention(message: result.message)
            } else {
                self.finishCapture(PostResult(ok: false, message: result.message, status: result.status))
            }
        }
    }

    @objc func showDoctor() {
        DialogPresenter.show(title: "Starlee Diagnostics", message: client.run(["doctor"]))
    }

    @objc private func showLastCaptureTrace() {
        let raw = client.run(["diagnostics", "--last-capture"])
        DialogPresenter.showTrace(
            title: "Last Capture Trace",
            summary: Self.captureTraceSummary(rawJSON: raw),
            rawJSON: raw
        )
    }

    private func hasLastCaptureTrace() -> Bool {
        guard let trace = client.runJSON(["diagnostics", "--last-capture"]) else { return false }
        return (trace["request_id"] as? String)?.isEmpty == false || (trace["events"] as? [[String: Any]])?.isEmpty == false
    }

    static func captureTraceSummary(rawJSON: String) -> String {
        guard
            let data = rawJSON.data(using: .utf8),
            let trace = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return "Summary\nTrace could not be parsed. Raw diagnostic output is shown below."
        }
        let request = trace["request_status"] as? [String: Any]
        let requested = request?["requested_browser"] as? String ?? trace["requested_browser"] as? String ?? "unknown"
        let handling = request?["handling_browser"] as? String ?? trace["handling_browser"] as? String ?? trace["browser"] as? String ?? "unknown"
        let result = trace["result_code"] as? String ?? trace["terminal_status"] as? String ?? "in_progress"
        let message = trace["user_safe_message"] as? String ?? "No user-safe message recorded."
        let next = trace["next_action"] as? String ?? trace["recommended_next_action"] as? String ?? "Run setup diagnostics."
        let events = trace["events"] as? [[String: Any]] ?? []
        let pageType = events
            .compactMap { event -> String? in
                let safe = event["safe_metadata"] as? [String: Any]
                return safe?["page_type"] as? String ?? safe?["payload_type"] as? String
            }
            .last ?? "unknown"
        return """
        Summary
        Requested browser: \(requested)
        Handling browser: \(handling)
        Page type: \(pageType)
        Result: \(result)
        Message: \(message)
        Next action: \(next)
        """
    }

    @objc func openVault() {
        NSWorkspace.shared.open(client.home.appendingPathComponent("vault"))
    }

    @objc private func startEngine() {
        client.startEngine()
    }

    @objc private func stopEngine() {
        client.stopEngine()
    }

    @objc private func quit() {
        client.stopEngine()
        NSApplication.shared.terminate(nil)
    }
}
