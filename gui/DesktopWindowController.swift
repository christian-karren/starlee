import AppKit

final class DesktopWindowController: NSWindowController {
    private let client: StarleeClient
    private weak var menuController: StatusMenuController?
    private let captureStatusValue = NSTextField(labelWithString: "Checking...")
    private let endpointStatusValue = NSTextField(labelWithString: "Checking...")
    private let extensionStatusValue = NSTextField(labelWithString: "Checking...")
    private let summaryLabel = NSTextField(wrappingLabelWithString: "Starlee is checking your local capture setup.")

    init(client: StarleeClient, menuController: StatusMenuController) {
        self.client = client
        self.menuController = menuController
        let screenFrame = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1280, height: 820)
        let window = NSWindow(
            contentRect: screenFrame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Starlee"
        window.minSize = NSSize(width: 820, height: 560)
        window.collectionBehavior.insert(.fullScreenPrimary)
        window.isReleasedWhenClosed = false
        super.init(window: window)
        window.contentView = makeContentView()
        refreshStatus()
    }

    required init?(coder: NSCoder) {
        nil
    }

    override func showWindow(_ sender: Any?) {
        super.showWindow(sender)
        if let visibleFrame = window?.screen?.visibleFrame ?? NSScreen.main?.visibleFrame {
            window?.setFrame(visibleFrame, display: true)
        }
        refreshStatus()
    }

    private func makeContentView() -> NSView {
        let root = NSView()
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor

        let brandIcon = NSImageView()
        brandIcon.image = NSImage(named: "StarleeDesktopIcon") ?? NSApp.applicationIconImage
        brandIcon.imageScaling = .scaleProportionallyUpOrDown
        brandIcon.translatesAutoresizingMaskIntoConstraints = false
        brandIcon.widthAnchor.constraint(equalToConstant: 88).isActive = true
        brandIcon.heightAnchor.constraint(equalToConstant: 88).isActive = true

        let title = NSTextField(labelWithString: "Starlee")
        title.font = .systemFont(ofSize: 40, weight: .bold)
        title.textColor = .labelColor

        let subtitle = NSTextField(wrappingLabelWithString: "Local-first capture, search, and retrieval for your personal knowledge vault.")
        subtitle.font = .systemFont(ofSize: 17, weight: .regular)
        subtitle.textColor = .secondaryLabelColor

        let brandStack = NSStackView(views: [brandIcon, title, subtitle])
        brandStack.orientation = .vertical
        brandStack.alignment = .leading
        brandStack.spacing = 14

        let captureCard = statusCard(title: "Capture System", value: captureStatusValue)
        let endpointCard = statusCard(title: "Local Endpoint", value: endpointStatusValue)
        let extensionCard = statusCard(title: "Browser Extension", value: extensionStatusValue)
        let statusStack = NSStackView(views: [captureCard, endpointCard, extensionCard])
        statusStack.orientation = .vertical
        statusStack.spacing = 12
        statusStack.distribution = .fillEqually

        summaryLabel.font = .systemFont(ofSize: 15)
        summaryLabel.textColor = .secondaryLabelColor

        let openVaultButton = NSButton(title: "Open Vault", target: self, action: #selector(openVault))
        openVaultButton.bezelStyle = .rounded
        openVaultButton.controlSize = .large

        let diagnosticsButton = NSButton(title: "Run Diagnostics", target: self, action: #selector(runDiagnostics))
        diagnosticsButton.bezelStyle = .rounded
        diagnosticsButton.controlSize = .large

        let refreshButton = NSButton(title: "Refresh", target: self, action: #selector(refresh))
        refreshButton.bezelStyle = .rounded
        refreshButton.controlSize = .large

        let buttonStack = NSStackView(views: [openVaultButton, diagnosticsButton, refreshButton])
        buttonStack.orientation = .horizontal
        buttonStack.spacing = 10

        let rightStack = NSStackView(views: [statusStack, summaryLabel, buttonStack])
        rightStack.orientation = .vertical
        rightStack.alignment = .leading
        rightStack.spacing = 18

        let contentStack = NSStackView(views: [brandStack, rightStack])
        contentStack.orientation = .horizontal
        contentStack.alignment = .centerY
        contentStack.spacing = 48
        contentStack.translatesAutoresizingMaskIntoConstraints = false
        contentStack.distribution = .fillEqually

        root.addSubview(contentStack)
        NSLayoutConstraint.activate([
            contentStack.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 64),
            contentStack.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -64),
            contentStack.topAnchor.constraint(equalTo: root.topAnchor, constant: 54),
            contentStack.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -54)
        ])
        return root
    }

    private func statusCard(title: String, value: NSTextField) -> NSView {
        let titleLabel = NSTextField(labelWithString: title)
        titleLabel.font = .systemFont(ofSize: 13, weight: .semibold)
        titleLabel.textColor = .secondaryLabelColor
        value.font = .systemFont(ofSize: 20, weight: .semibold)
        value.textColor = .labelColor

        let stack = NSStackView(views: [titleLabel, value])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false

        let card = NSView()
        card.wantsLayer = true
        card.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        card.layer?.cornerRadius = 8
        card.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 18),
            stack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -18),
            stack.topAnchor.constraint(equalTo: card.topAnchor, constant: 16),
            stack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -16)
        ])
        return card
    }

    private func refreshStatus() {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            let doctor = self.client.runJSON(["doctor"])
            let status = doctor?["status"] as? [String: Any]
            let checks = doctor?["checks"] as? [[String: Any]] ?? []
            let captureCount = (status?["capture_count"] as? NSNumber)?.intValue ?? 0
            let captureReady = self.checksAreOK(
                checks,
                names: ["vault", "index", "token", "extension_assets"]
            )
            let endpointReady = self.client.healthCheck()
            let browserBridge = status?["bridge_health"] as? [String: Any]
            let bridgeReady = (browserBridge?["ok"] as? Bool) == true
            let nextAction = (browserBridge?["recommended_next_action"] as? String)
                ?? self.firstFailedCheckDetail(in: checks)
                ?? "Run setup if this is your first launch."

            DispatchQueue.main.async {
                self.captureStatusValue.stringValue = captureReady ? "\(captureCount) captures · ready" : "\(captureCount) captures · needs setup"
                self.captureStatusValue.textColor = captureReady ? .systemGreen : .systemOrange
                self.endpointStatusValue.stringValue = endpointReady ? "Reachable on 127.0.0.1" : "Not running"
                self.endpointStatusValue.textColor = endpointReady ? .systemGreen : .systemOrange
                self.extensionStatusValue.stringValue = bridgeReady ? "Connected" : "Needs attention"
                self.extensionStatusValue.textColor = bridgeReady ? .systemGreen : .systemOrange
                self.summaryLabel.stringValue = nextAction
            }
        }
    }

    private func firstFailedCheckDetail(in checks: [[String: Any]]) -> String? {
        checks.first { ($0["ok"] as? Bool) == false }?["detail"] as? String
    }

    private func checksAreOK(_ checks: [[String: Any]], names: Set<String>) -> Bool {
        let matchingChecks = checks.filter { check in
            guard let name = check["name"] as? String else { return false }
            return names.contains(name)
        }
        return !matchingChecks.isEmpty && matchingChecks.allSatisfy { ($0["ok"] as? Bool) == true }
    }

    @objc private func openVault() {
        menuController?.openVault()
    }

    @objc private func runDiagnostics() {
        menuController?.showDoctor()
        refreshStatus()
    }

    @objc private func refresh() {
        refreshStatus()
    }
}
