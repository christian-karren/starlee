import AppKit
import UniformTypeIdentifiers
import WebKit

final class DesktopWindowController: NSWindowController, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate, WKNavigationDelegate, WKScriptMessageHandler {
    private enum PrimaryView {
        case library
        case settings
    }

    private struct LibraryCapture {
        let id: String
        let title: String
        let type: String
        let site: String?
        let author: String?
        let url: URL?
        let capturedAt: Date?
        let capturedAtText: String
        let filePath: String
        let snippet: String
        let topics: [String]

        var monthKey: String {
            guard let capturedAt else { return "undated" }
            return Self.monthKeyFormatter.string(from: capturedAt)
        }

        var monthLabel: String {
            guard let capturedAt else { return "Undated" }
            return Self.monthLabelFormatter.string(from: capturedAt)
        }

        var source: String {
            if let host = url?.host, !host.isEmpty {
                return host.replacingOccurrences(of: "www.", with: "")
            }
            if let site, !site.isEmpty { return site }
            return URL(fileURLWithPath: filePath).lastPathComponent
        }

        var transcriptStatus: String {
            guard type == "youtube" else { return "" }
            let lower = snippet.lowercased()
            if lower.contains("transcript unavailable") { return "Transcript unavailable" }
            if lower.contains("metadata only") { return "Metadata only" }
            if lower.contains("transcript") { return "Transcript" }
            return "Metadata only"
        }

        private static let monthKeyFormatter: DateFormatter = {
            let formatter = DateFormatter()
            formatter.calendar = Calendar(identifier: .gregorian)
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.dateFormat = "yyyy-MM"
            return formatter
        }()

        private static let monthLabelFormatter: DateFormatter = {
            let formatter = DateFormatter()
            formatter.calendar = Calendar(identifier: .gregorian)
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.dateFormat = "MMMM yyyy"
            return formatter
        }()
    }

    private struct MonthGroup {
        let id: String
        let label: String
        let captures: [LibraryCapture]
    }

    private let client: StarleeClient
    private weak var menuController: StatusMenuController?
    private let fluidBackgroundStore = FluidBackgroundSettingsStore()
    private var primaryView: PrimaryView = .library
    private var doctor: [String: Any]?
    private var captures: [LibraryCapture] = []
    private var groups: [MonthGroup] = []
    private var filteredCaptures: [LibraryCapture] = []
    private var selectedMonthID: String?
    private lazy var fluidBackground = fluidBackgroundStore.load()

    private let sidebarBackground = SidebarHoleBackgroundView()
    private let libraryButton = SidebarBoxButton(title: "Library")
    private let settingsButton = SidebarBoxButton(title: "Settings")
    private let monthStack = NSStackView()
    private var monthButtons: [String: NSButton] = [:]
    private weak var sidebarDivider: NSView?
    private var appBackgroundWebView: WKWebView?
    private weak var rootSplitView: NSSplitView?
    private weak var pixelColorWell: NSColorWell?
    private weak var backgroundColorWell: NSColorWell?
    private weak var blackColorWell: NSColorWell?
    private weak var whiteColorWell: NSColorWell?
    private weak var pixelSizeSlider: NSSlider?
    private weak var thresholdSlider: NSSlider?
    private weak var fluidSpeedSlider: NSSlider?
    private weak var zoomSlider: NSSlider?
    private weak var pixelSizeValueLabel: NSTextField?
    private weak var thresholdValueLabel: NSTextField?
    private weak var fluidSpeedValueLabel: NSTextField?
    private weak var zoomValueLabel: NSTextField?
    private var headerView: NSView?
    private let titleLabel = NSTextField(labelWithString: "Library")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private let readinessLabel = NSTextField(wrappingLabelWithString: "")
    private let searchField = NSSearchField()
    private static let onboardingCompleteKey = "StarleeOnboardingComplete"
    private let tableView = NSTableView()
    private var libraryWebView: WKWebView?
    private var libraryWebViewLoaded = false
    private var pendingLibraryPayload: String?
    private var settingsWebView: WKWebView?
    private var settingsWebViewLoaded = false
    private var pendingSettingsPayload: String?
    private var automaticRefreshTimer: Timer?
    private var isReloading = false
    private let openButton = NSButton(title: "Open Original", target: nil, action: nil)
    private let revealButton = NSButton(title: "Reveal File", target: nil, action: nil)
    private let importButton = NSButton(title: "Import", target: nil, action: nil)
    private let contentStack = NSStackView()
    private let progress = NSProgressIndicator()
    private static let starleeBlack = NSColor(calibratedWhite: 0, alpha: 1)
    private static let starleeWhite = NSColor(calibratedWhite: 1, alpha: 1)
    private static let starleeCream = NSColor(calibratedRed: 0.949, green: 0.890, blue: 0.714, alpha: 1)
    private static let starleeNavy = NSColor(calibratedRed: 0.075, green: 0.157, blue: 0.294, alpha: 1)

    init(client: StarleeClient, menuController: StatusMenuController) {
        self.client = client
        self.menuController = menuController
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1080, height: 720),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = ""
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.styleMask.insert(.fullSizeContentView)
        window.backgroundColor = .clear
        window.isOpaque = false
        window.isMovableByWindowBackground = true
        window.minSize = NSSize(width: 900, height: 620)
        window.collectionBehavior.insert(.fullScreenPrimary)
        window.isReleasedWhenClosed = false
        super.init(window: window)
        window.contentView = makeContentView()
        applyFluidBackground()
        window.center()
        reload()
        startAutomaticRefresh()
    }

    required init?(coder: NSCoder) {
        nil
    }

    override func showWindow(_ sender: Any?) {
        super.showWindow(sender)
        window?.makeKeyAndOrderFront(sender)
        reload()
    }

    deinit {
        automaticRefreshTimer?.invalidate()
    }

    private func makeContentView() -> NSView {
        let split = NSSplitView()
        split.isVertical = true
        split.dividerStyle = .thin
        split.translatesAutoresizingMaskIntoConstraints = false
        split.wantsLayer = true
        split.layer?.backgroundColor = NSColor.clear.cgColor
        rootSplitView = split

        let sidebar = makeSidebar()
        let main = makeMainPane()
        split.addArrangedSubview(sidebar)
        split.addArrangedSubview(main)
        split.setPosition(300, ofDividerAt: 0)

        let root = NSView()
        let background = makeAppBackgroundWebView()
        appBackgroundWebView = background
        root.addSubview(background)
        root.addSubview(split)
        NSLayoutConstraint.activate([
            background.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            background.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            background.topAnchor.constraint(equalTo: root.topAnchor),
            background.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            split.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            split.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            split.topAnchor.constraint(equalTo: root.topAnchor),
            split.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            sidebar.widthAnchor.constraint(equalToConstant: 300)
        ])
        return root
    }

    private func makeAppBackgroundWebView() -> WKWebView {
        let webView = WKWebView(frame: .zero, configuration: WKWebViewConfiguration())
        webView.navigationDelegate = self
        webView.translatesAutoresizingMaskIntoConstraints = false
        webView.setValue(false, forKey: "drawsBackground")
        webView.isHidden = false
        if let url = Bundle.main.url(forResource: "background", withExtension: "html", subdirectory: "renderer") {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }
        return webView
    }

    private func makeSidebar() -> NSView {
        // Solid-black sidebar that knocks out button-shaped holes so the
        // full-window app background WebView (behind the split view) shows
        // through the three nav plaques only — the black field elsewhere stays
        // intact. See SidebarHoleBackgroundView.
        let sidebar = sidebarBackground
        sidebar.translatesAutoresizingMaskIntoConstraints = false
        sidebar.wantsLayer = true

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .width
        stack.spacing = 28
        stack.edgeInsets = NSEdgeInsets(top: 30, left: 20, bottom: 20, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        sidebar.addSubview(stack)

        let wordmark = NSImageView()
        wordmark.image = Bundle.main.url(forResource: "StarleeWordmark", withExtension: "png")
            .flatMap(NSImage.init(contentsOf:))
        wordmark.imageScaling = .scaleProportionallyUpOrDown
        wordmark.translatesAutoresizingMaskIntoConstraints = false
        wordmark.heightAnchor.constraint(equalToConstant: 106).isActive = true
        stack.addArrangedSubview(wordmark)

        configureSidebarButton(libraryButton, action: #selector(showLibrary))
        configureSidebarButton(settingsButton, action: #selector(showSettings))

        let navStack = NSStackView(views: [libraryButton, settingsButton])
        navStack.orientation = .vertical
        navStack.alignment = .width
        navStack.spacing = 22
        stack.addArrangedSubview(navStack)

        let divider = NSView()
        divider.wantsLayer = true
        divider.layer?.backgroundColor = NSColor(calibratedRed: 0.949, green: 0.890, blue: 0.714, alpha: 0.86).cgColor
        divider.translatesAutoresizingMaskIntoConstraints = false
        divider.heightAnchor.constraint(equalToConstant: 1).isActive = true
        divider.isHidden = true
        stack.addArrangedSubview(divider)
        sidebarDivider = divider

        monthStack.orientation = .vertical
        monthStack.alignment = .width
        monthStack.spacing = 22
        stack.addArrangedSubview(monthStack)
        stack.addArrangedSubview(NSView())

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
            stack.topAnchor.constraint(equalTo: sidebar.topAnchor),
            stack.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor)
        ])
        refreshSidebarHoles()
        return sidebar
    }

    /// Tells the sidebar which button plaques to cut background-revealing holes
    /// behind. Called after the nav buttons exist and whenever the month
    /// buttons are rebuilt.
    private func refreshSidebarHoles() {
        var buttons: [SidebarBoxButton] = [libraryButton, settingsButton]
        buttons.append(contentsOf: monthButtons.values.compactMap { $0 as? SidebarBoxButton })
        sidebarBackground.knockoutButtons = buttons
    }

    private func makeMainPane() -> NSView {
        let main = NSView()
        main.translatesAutoresizingMaskIntoConstraints = false
        main.wantsLayer = true
        main.layer?.backgroundColor = NSColor.clear.cgColor

        contentStack.orientation = .vertical
        contentStack.alignment = .leading
        contentStack.spacing = 14
        contentStack.edgeInsets = NSEdgeInsets(top: 52, left: 24, bottom: 20, right: 24)
        contentStack.translatesAutoresizingMaskIntoConstraints = false
        main.addSubview(contentStack)

        let header = NSStackView()
        header.orientation = .horizontal
        header.alignment = .centerY
        header.spacing = 12

        let titleBox = NSBox()
        titleBox.boxType = .custom
        titleBox.cornerRadius = 0
        titleBox.borderWidth = 3
        titleBox.borderColor = Self.starleeBlack
        titleBox.fillColor = Self.starleeWhite
        titleBox.translatesAutoresizingMaskIntoConstraints = false
        titleBox.wantsLayer = true
        titleBox.layer?.shadowColor = Self.starleeBlack.cgColor
        titleBox.layer?.shadowOpacity = 0.72
        titleBox.layer?.shadowRadius = 0
        titleBox.layer?.shadowOffset = NSSize(width: 6, height: -6)

        titleLabel.font = .systemFont(ofSize: 34, weight: .heavy)
        titleLabel.textColor = Self.starleeBlack
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleBox.addSubview(titleLabel)
        NSLayoutConstraint.activate([
            titleBox.widthAnchor.constraint(greaterThanOrEqualToConstant: 320),
            titleBox.heightAnchor.constraint(equalToConstant: 58),
            titleLabel.leadingAnchor.constraint(equalTo: titleBox.leadingAnchor, constant: 24),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: titleBox.trailingAnchor, constant: -24),
            titleLabel.centerYAnchor.constraint(equalTo: titleBox.centerYAnchor)
        ])

        progress.style = .spinning
        progress.controlSize = .small
        progress.isDisplayedWhenStopped = false

        header.addArrangedSubview(titleBox)
        header.addArrangedSubview(NSView())
        header.addArrangedSubview(progress)
        headerView = header
        contentStack.addArrangedSubview(header)
        header.widthAnchor.constraint(equalTo: contentStack.widthAnchor).isActive = true

        readinessLabel.font = .systemFont(ofSize: 13)
        readinessLabel.textColor = .secondaryLabelColor
        readinessLabel.isHidden = true
        contentStack.addArrangedSubview(readinessLabel)

        configureTable()

        NSLayoutConstraint.activate([
            contentStack.leadingAnchor.constraint(equalTo: main.leadingAnchor),
            contentStack.trailingAnchor.constraint(equalTo: main.trailingAnchor),
            contentStack.topAnchor.constraint(equalTo: main.topAnchor),
            contentStack.bottomAnchor.constraint(equalTo: main.bottomAnchor)
        ])
        return main
    }

    private func configureSidebarButton(_ button: NSButton, action: Selector) {
        button.target = self
        button.action = action
    }

    private func configureTable() {
        tableView.delegate = self
        tableView.dataSource = self
        tableView.rowHeight = 42
        tableView.usesAlternatingRowBackgroundColors = true
        addColumn("title", "Title", width: 310)
        addColumn("source", "Source", width: 140)
        addColumn("type", "Type", width: 92)
        addColumn("captured", "Captured", width: 120)
        addColumn("transcript", "Transcript", width: 150)
        addColumn("file", "Vault File", width: 180)
    }

    private func render() {
        removeContent(afterHeader: true)
        updateSidebarSelection()
        // Both Library and Settings render full-bleed in their own WebView, so
        // the native header stays hidden and the content fills the pane.
        headerView?.isHidden = true
        contentStack.spacing = 0
        contentStack.edgeInsets = NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
        renderReadiness()
        switch primaryView {
        case .library:
            renderLibrary()
        case .settings:
            renderSettings()
        }
    }

    private func removeContent(afterHeader: Bool) {
        let preserved = afterHeader ? 2 : 0
        while contentStack.arrangedSubviews.count > preserved {
            let view = contentStack.arrangedSubviews[preserved]
            contentStack.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
    }

    private func renderReadiness() {
        readinessLabel.isHidden = true
        readinessLabel.stringValue = ""
    }

    private func renderLibrary() {
        let webView = libraryWebView ?? makeLibraryWebView()
        libraryWebView = webView
        contentStack.addArrangedSubview(webView)
        NSLayoutConstraint.activate([
            webView.leadingAnchor.constraint(equalTo: contentStack.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: contentStack.trailingAnchor),
            webView.topAnchor.constraint(equalTo: contentStack.topAnchor),
            webView.bottomAnchor.constraint(equalTo: contentStack.bottomAnchor)
        ])

        if webView.url == nil {
            loadLibraryRenderer(webView)
        }
        renderLibraryPayload()
        updateActionButtons()
    }

    private func makeLibraryWebView() -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.userContentController.add(self, name: "starlee")
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = self
        webView.translatesAutoresizingMaskIntoConstraints = false
        webView.allowsMagnification = false
        webView.setValue(false, forKey: "drawsBackground")
        return webView
    }

    private func loadLibraryRenderer(_ webView: WKWebView) {
        guard let rendererURL = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "renderer") else {
            return
        }
        let readAccess = rendererURL.deletingLastPathComponent()
        webView.loadFileURL(rendererURL, allowingReadAccessTo: readAccess)
    }

    private func renderLibraryPayload() {
        let payload = libraryPayloadJSON()
        guard libraryWebViewLoaded, let webView = libraryWebView else {
            pendingLibraryPayload = payload
            return
        }
        webView.evaluateJavaScript("window.__starleeLibraryPayload = \(payload); if (window.renderStarleeLibrary) { window.renderStarleeLibrary(window.__starleeLibraryPayload); }", completionHandler: nil)
    }

    private func renderSettings() {
        let webView = settingsWebView ?? makeSettingsWebView()
        settingsWebView = webView
        contentStack.addArrangedSubview(webView)
        NSLayoutConstraint.activate([
            webView.leadingAnchor.constraint(equalTo: contentStack.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: contentStack.trailingAnchor),
            webView.topAnchor.constraint(equalTo: contentStack.topAnchor),
            webView.bottomAnchor.constraint(equalTo: contentStack.bottomAnchor)
        ])
        if webView.url == nil {
            loadSettingsRenderer(webView)
        }
        renderSettingsPayload()
    }

    private func makeSettingsWebView() -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.userContentController.add(self, name: "starlee")
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = self
        webView.translatesAutoresizingMaskIntoConstraints = false
        webView.allowsMagnification = false
        webView.setValue(false, forKey: "drawsBackground")
        return webView
    }

    private func loadSettingsRenderer(_ webView: WKWebView) {
        guard let rendererURL = Bundle.main.url(forResource: "settings", withExtension: "html", subdirectory: "renderer") else {
            return
        }
        webView.loadFileURL(rendererURL, allowingReadAccessTo: rendererURL.deletingLastPathComponent())
    }

    private func renderSettingsPayload() {
        let payload = settingsPayloadJSON()
        guard settingsWebViewLoaded, let webView = settingsWebView else {
            pendingSettingsPayload = payload
            return
        }
        webView.evaluateJavaScript("window.__starleeSettingsPayload = \(payload); if (window.renderStarleeSettings) { window.renderStarleeSettings(window.__starleeSettingsPayload); }", completionHandler: nil)
    }

    private func settingsPayloadJSON() -> String {
        let checks = checksByName()
        let bridge = (status()["bridge_health"] as? [String: Any]) ?? [:]
        let chromeSetup = bridge["chrome_setup"] as? [String: Any] ?? [:]
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
        let bridgeOK = (bridge["ok"] as? Bool) ?? false
        let codexOK = checks["codex_plugin_source"]?.ok == true
        let diagOK = doctor?["ok"] as? Bool == true
        let vaultOK = checks["vault"]?.ok == true

        let sections: [[String: Any]] = [
            [
                "id": "browser", "title": "Browser extension",
                "status": browserSetupStatus(chromeSetup, bridge: bridge), "ok": bridgeOK,
                "action": "openBrowserSetup", "actionLabel": "Open setup"
            ],
            [
                "id": "codex", "title": "Codex plugin",
                "status": codexOK ? "Installed" : "Needs setup", "ok": codexOK,
                "action": "codexGuide", "actionLabel": "Guide"
            ],
            [
                "id": "diagnostics", "title": "Diagnostics",
                "status": diagOK ? "Ready" : "Needs attention", "ok": diagOK,
                "action": "copyDiagnostics", "actionLabel": "Copy redacted"
            ],
            [
                "id": "vault", "title": "Vault",
                "status": vaultOK ? "Local" : "Missing", "ok": vaultOK,
                "detail": statusString("vault"),
                "action": "openVault", "actionLabel": "Open"
            ],
            [
                "id": "upload", "title": "Upload documents",
                "status": "PDF · Word · text", "ok": true,
                "action": "upload", "actionLabel": "Upload"
            ],
            [
                "id": "export", "title": "Share your brain",
                "status": "Audited export", "ok": true,
                "detail": "Creates a shareable copy. Restricted article bodies are always removed.",
                "action": "exportBrain", "actionLabel": "Export"
            ],
            [
                "id": "ingest", "title": "Borrow a brain",
                "status": "Read-only", "ok": true,
                "detail": "Open a friend’s .starlee bundle and search it with scope: borrowed.",
                "action": "ingestBrain", "actionLabel": "Ingest"
            ],
            [
                "id": "onboarding", "title": "Getting started",
                "status": "Replay the intro", "ok": true,
                "action": "rerunOnboarding", "actionLabel": "Show me"
            ],
            [
                "id": "privacy", "title": "Privacy",
                "status": "On-device", "ok": true,
                "detail": "Captured content and search run locally. Nothing leaves your device unless you export it."
            ],
            [
                "id": "about", "title": "App version",
                "status": version, "ok": true
            ]
        ]
        let payload: [String: Any] = [
            "background": fluidBackground.webPayload,
            "sections": sections
        ]
        guard
            JSONSerialization.isValidJSONObject(payload),
            let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
            let json = String(data: data, encoding: .utf8)
        else {
            return #"{"background":{},"sections":[]}"#
        }
        return json
    }

    private func appearancePanel() -> NSView {
        let box = NSBox()
        box.boxType = .custom
        box.cornerRadius = 8
        box.borderWidth = 2
        box.borderColor = Self.starleeWhite.withAlphaComponent(0.48)
        box.fillColor = Self.starleeNavy.withAlphaComponent(0.85)
        box.translatesAutoresizingMaskIntoConstraints = false
        box.wantsLayer = true
        box.layer?.shadowColor = Self.starleeBlack.cgColor
        box.layer?.shadowOpacity = 0.32
        box.layer?.shadowRadius = 16
        box.layer?.shadowOffset = NSSize(width: 0, height: -8)
        box.widthAnchor.constraint(greaterThanOrEqualToConstant: 620).isActive = true

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 18
        stack.edgeInsets = NSEdgeInsets(top: 24, left: 24, bottom: 24, right: 24)
        stack.translatesAutoresizingMaskIntoConstraints = false
        box.addSubview(stack)

        let kind = fluidBackground.kind
        let usesPalette = kind == "aurora" || kind == "dither" || kind == "glass"

        let titleStack = NSStackView()
        titleStack.orientation = .vertical
        titleStack.spacing = 4
        let title = NSTextField(labelWithString: "Background")
        title.font = .systemFont(ofSize: 24, weight: .heavy)
        title.textColor = Self.starleeWhite
        let subtitle = NSTextField(labelWithString: subtitleText(for: kind))
        subtitle.font = .systemFont(ofSize: 13, weight: .semibold)
        subtitle.textColor = Self.starleeCream
        titleStack.addArrangedSubview(title)
        titleStack.addArrangedSubview(subtitle)
        stack.addArrangedSubview(titleStack)

        // Background suite: a gallery of looks spanning all engines.
        stack.addArrangedSubview(captionLabel("Style"))
        stack.addArrangedSubview(lookGallery())

        // Colors. The aurora/dither/glass engines use the full four-color
        // palette; pixel-dither and flow use just the two-color pair.
        if usesPalette {
            stack.addArrangedSubview(paletteRow())
        } else {
            let colorRow = NSStackView()
            colorRow.orientation = .horizontal
            colorRow.alignment = .centerY
            colorRow.spacing = 18
            let isFlow = kind == "flow"
            let pixelColor = colorControl(title: isFlow ? "Navy" : "Pixel color", hex: fluidBackground.pixelColor, action: #selector(changePixelColor(_:)))
            pixelColorWell = pixelColor.well
            let backgroundColor = colorControl(title: isFlow ? "Cream" : "Background color", hex: fluidBackground.backgroundColor, action: #selector(changeBackgroundColor(_:)))
            backgroundColorWell = backgroundColor.well
            colorRow.addArrangedSubview(pixelColor.view)
            colorRow.addArrangedSubview(backgroundColor.view)
            stack.addArrangedSubview(colorRow)
        }

        // Engine-specific controls.
        switch kind {
        case "flow": stack.addArrangedSubview(flowControls())
        case "aurora": stack.addArrangedSubview(auroraControls())
        case "dither": stack.addArrangedSubview(ditherControls())
        case "glass": stack.addArrangedSubview(glassControls())
        default: stack.addArrangedSubview(pixelControls())
        }

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: box.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: box.trailingAnchor),
            stack.topAnchor.constraint(equalTo: box.topAnchor),
            stack.bottomAnchor.constraint(equalTo: box.bottomAnchor)
        ])

        updateFluidBackgroundControls()
        return box
    }

    private func captionLabel(_ text: String) -> NSTextField {
        let label = NSTextField(labelWithString: text.uppercased())
        label.font = .systemFont(ofSize: 11, weight: .heavy)
        label.textColor = Self.starleeCream
        return label
    }

    private func lookGallery() -> NSView {
        let column = NSStackView()
        column.orientation = .vertical
        column.alignment = .leading
        column.spacing = 8
        var row: NSStackView?
        for (index, look) in FluidBackgroundLooks.all.enumerated() {
            if index % 4 == 0 {
                let newRow = NSStackView()
                newRow.orientation = .horizontal
                newRow.alignment = .centerY
                newRow.spacing = 8
                column.addArrangedSubview(newRow)
                row = newRow
            }
            row?.addArrangedSubview(makeLookTile(look))
        }
        return column
    }

    private func makeLookTile(_ look: FluidBackgroundLook) -> NSButton {
        let button = NSButton(title: look.name, target: self, action: #selector(selectFluidLook(_:)))
        styleSettingsActionButton(button)
        button.identifier = NSUserInterfaceItemIdentifier(look.name)
        if lookMatchesCurrent(look) {
            // Active look reads as selected: navy fill + white label.
            button.layer?.backgroundColor = Self.starleeNavy.withAlphaComponent(0.95).cgColor
            button.layer?.borderColor = Self.starleeWhite.cgColor
            button.attributedTitle = NSAttributedString(
                string: look.name,
                attributes: [
                    .foregroundColor: Self.starleeWhite,
                    .font: NSFont.systemFont(ofSize: 12, weight: .heavy)
                ]
            )
        } else {
            button.attributedTitle = NSAttributedString(
                string: look.name,
                attributes: [
                    .foregroundColor: Self.starleeBlack,
                    .font: NSFont.systemFont(ofSize: 12, weight: .bold)
                ]
            )
        }
        return button
    }

    private func lookMatchesCurrent(_ look: FluidBackgroundLook) -> Bool {
        guard look.settings.kind == fluidBackground.kind else { return false }
        // Two pixel-dither looks share a palette and differ only by threshold;
        // each of the other engines has a single preset.
        if look.settings.kind == "pixel-dither" {
            return abs(fluidBackground.threshold - look.settings.threshold) < 0.001
                && fluidBackground.pixelColor.uppercased() == look.settings.pixelColor.uppercased()
        }
        return true
    }

    private func flowFinishIndex(_ finish: String) -> Int {
        switch finish {
        case "sharp": return 0
        case "glass": return 2
        default: return 1
        }
    }

    private func flowControls() -> NSView {
        let controls = NSStackView()
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 12

        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 12
        let finishLabel = NSTextField(labelWithString: "Finish")
        finishLabel.font = .systemFont(ofSize: 12, weight: .bold)
        finishLabel.textColor = Self.starleeWhite
        finishLabel.widthAnchor.constraint(equalToConstant: 110).isActive = true
        let segmented = NSSegmentedControl(
            labels: ["Sharp", "Soft", "Glass"],
            trackingMode: .selectOne,
            target: self,
            action: #selector(selectFlowFinish(_:))
        )
        segmented.selectedSegment = flowFinishIndex(fluidBackground.flowFinish)
        let randomize = NSButton(title: "Randomize", target: self, action: #selector(randomizeSeed))
        styleSettingsActionButton(randomize)
        row.addArrangedSubview(finishLabel)
        row.addArrangedSubview(segmented)
        row.addArrangedSubview(randomize)
        controls.addArrangedSubview(row)

        let speedRow = sliderRow(
            title: "Speed",
            value: fluidBackground.speed,
            min: 0.005,
            max: 0.08,
            action: #selector(changeFluidSpeed(_:))
        )
        fluidSpeedSlider = speedRow.slider
        fluidSpeedValueLabel = speedRow.valueLabel
        controls.addArrangedSubview(speedRow.view)
        return controls
    }

    private func pixelControls() -> NSView {
        let controls = NSStackView()
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 9

        let pixelSizeRow = sliderRow(
            title: "Pixel size",
            value: fluidBackground.pixelSize,
            min: 1,
            max: 12,
            action: #selector(changePixelSize(_:))
        )
        pixelSizeSlider = pixelSizeRow.slider
        pixelSizeValueLabel = pixelSizeRow.valueLabel

        let thresholdRow = sliderRow(
            title: "Threshold",
            value: fluidBackground.threshold,
            min: 0.12,
            max: 0.55,
            action: #selector(changeThreshold(_:))
        )
        thresholdSlider = thresholdRow.slider
        thresholdValueLabel = thresholdRow.valueLabel

        let speedRow = sliderRow(
            title: "Speed",
            value: fluidBackground.speed,
            min: 0.005,
            max: 0.08,
            action: #selector(changeFluidSpeed(_:))
        )
        fluidSpeedSlider = speedRow.slider
        fluidSpeedValueLabel = speedRow.valueLabel

        let zoomRow = sliderRow(
            title: "Zoom",
            value: fluidBackground.zoom,
            min: 2,
            max: 7,
            action: #selector(changeZoom(_:))
        )
        zoomSlider = zoomRow.slider
        zoomValueLabel = zoomRow.valueLabel

        controls.addArrangedSubview(pixelSizeRow.view)
        controls.addArrangedSubview(thresholdRow.view)
        controls.addArrangedSubview(speedRow.view)
        controls.addArrangedSubview(zoomRow.view)
        return controls
    }

    private func subtitleText(for kind: String) -> String {
        switch kind {
        case "flow": return "Flowing ribbon background · saved instantly"
        case "aurora": return "Aurora gradient background · saved instantly"
        case "dither": return "Animated halftone dither · saved instantly"
        case "glass": return "Glass background · saved instantly"
        default: return "Fluid pixel-dither background · saved instantly"
        }
    }

    /// Four-color palette row (navy / cream / black / white) for the aurora,
    /// dither, and glass engines.
    private func paletteRow() -> NSView {
        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 18
        let navy = paletteColor(title: "Navy", hex: fluidBackground.pixelColor, action: #selector(changePixelColor(_:)))
        pixelColorWell = navy.well
        let cream = paletteColor(title: "Cream", hex: fluidBackground.backgroundColor, action: #selector(changeBackgroundColor(_:)))
        backgroundColorWell = cream.well
        let black = paletteColor(title: "Black", hex: fluidBackground.black, action: #selector(changeBlackColor(_:)))
        blackColorWell = black.well
        let white = paletteColor(title: "White", hex: fluidBackground.white, action: #selector(changeWhiteColor(_:)))
        whiteColorWell = white.well
        row.addArrangedSubview(navy.view)
        row.addArrangedSubview(cream.view)
        row.addArrangedSubview(black.view)
        row.addArrangedSubview(white.view)
        return row
    }

    private func paletteColor(title: String, hex: String, action: Selector) -> (view: NSView, well: NSColorWell) {
        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 6
        let well = NSColorWell()
        well.color = FluidBackgroundSettings.color(from: hex)
        well.target = self
        well.action = action
        well.widthAnchor.constraint(equalToConstant: 38).isActive = true
        well.heightAnchor.constraint(equalToConstant: 24).isActive = true
        let label = NSTextField(labelWithString: title)
        label.font = .systemFont(ofSize: 12, weight: .semibold)
        label.textColor = Self.starleeCream
        row.addArrangedSubview(well)
        row.addArrangedSubview(label)
        return (row, well)
    }

    private func auroraControls() -> NSView {
        let controls = NSStackView()
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 10

        let intensityRow = sliderRow(title: "Intensity", value: fluidBackground.auroraIntensity, min: 0.2, max: 0.85, action: #selector(changeAuroraIntensity(_:)))
        controls.addArrangedSubview(intensityRow.view)

        let speedRow = sliderRow(title: "Speed", value: fluidBackground.speed, min: 0, max: 1.6, action: #selector(changeFluidSpeed(_:)))
        fluidSpeedSlider = speedRow.slider
        fluidSpeedValueLabel = speedRow.valueLabel
        controls.addArrangedSubview(speedRow.view)

        controls.addArrangedSubview(randomizeButton())
        return controls
    }

    private func ditherControls() -> NSView {
        let controls = NSStackView()
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 9

        let dotRow = sliderRow(title: "Dot size", value: fluidBackground.ditherDotSize, min: 3, max: 12, action: #selector(changeDitherDotSize(_:)))
        controls.addArrangedSubview(dotRow.view)
        let contrastRow = sliderRow(title: "Contrast", value: fluidBackground.ditherContrast, min: 0.7, max: 2.2, action: #selector(changeDitherContrast(_:)))
        controls.addArrangedSubview(contrastRow.view)
        let navyRow = sliderRow(title: "Navy buffer", value: fluidBackground.ditherNavyBuffer, min: 0.5, max: 2.5, action: #selector(changeDitherNavyBuffer(_:)))
        controls.addArrangedSubview(navyRow.view)
        let speedRow = sliderRow(title: "Speed", value: fluidBackground.speed, min: 0, max: 0.01, action: #selector(changeFluidSpeed(_:)))
        fluidSpeedSlider = speedRow.slider
        fluidSpeedValueLabel = speedRow.valueLabel
        controls.addArrangedSubview(speedRow.view)

        controls.addArrangedSubview(randomizeButton())
        return controls
    }

    private func glassControls() -> NSView {
        let controls = NSStackView()
        controls.orientation = .vertical
        controls.alignment = .leading
        controls.spacing = 9

        let modeRow = NSStackView()
        modeRow.orientation = .horizontal
        modeRow.alignment = .centerY
        modeRow.spacing = 12
        let modeLabel = NSTextField(labelWithString: "Mode")
        modeLabel.font = .systemFont(ofSize: 12, weight: .bold)
        modeLabel.textColor = Self.starleeWhite
        modeLabel.widthAnchor.constraint(equalToConstant: 110).isActive = true
        let segmented = NSSegmentedControl(labels: ["Panes", "Blur only"], trackingMode: .selectOne, target: self, action: #selector(selectGlassMode(_:)))
        segmented.selectedSegment = fluidBackground.glassMode == "blur" ? 1 : 0
        modeRow.addArrangedSubview(modeLabel)
        modeRow.addArrangedSubview(segmented)
        controls.addArrangedSubview(modeRow)

        let speedRow = sliderRow(title: "Speed", value: fluidBackground.speed, min: 0, max: 0.01, action: #selector(changeFluidSpeed(_:)))
        fluidSpeedSlider = speedRow.slider
        fluidSpeedValueLabel = speedRow.valueLabel
        controls.addArrangedSubview(speedRow.view)

        let softRow = sliderRow(title: "Softness", value: fluidBackground.glassSoftness, min: 0, max: 40, action: #selector(changeGlassSoftness(_:)))
        controls.addArrangedSubview(softRow.view)
        let brightRow = sliderRow(title: "Brightness", value: fluidBackground.glassBrightness, min: 0.6, max: 1.6, action: #selector(changeGlassBrightness(_:)))
        controls.addArrangedSubview(brightRow.view)

        if fluidBackground.glassMode != "blur" {
            let panesRow = sliderRow(title: "Panes", value: fluidBackground.glassPanes, min: 8, max: 32, action: #selector(changeGlassPanes(_:)))
            controls.addArrangedSubview(panesRow.view)
            let refrRow = sliderRow(title: "Refraction", value: fluidBackground.glassRefraction, min: 0, max: 0.05, action: #selector(changeGlassRefraction(_:)))
            controls.addArrangedSubview(refrRow.view)
        }

        controls.addArrangedSubview(randomizeButton())
        return controls
    }

    private func randomizeButton() -> NSView {
        let button = NSButton(title: "Randomize", target: self, action: #selector(randomizeSeed))
        styleSettingsActionButton(button)
        return button
    }

    /// Updates the value label of a slider built by `sliderRow` (it is the last
    /// arranged subview of the slider's parent stack).
    private func setRowValueLabel(_ slider: NSSlider, _ text: String) {
        guard let row = slider.superview as? NSStackView else { return }
        (row.arrangedSubviews.last as? NSTextField)?.stringValue = text
    }

    private func colorControl(title: String, hex: String, action: Selector) -> (view: NSView, well: NSColorWell) {
        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 8

        let label = NSTextField(labelWithString: title)
        label.font = .systemFont(ofSize: 12, weight: .bold)
        label.textColor = Self.starleeWhite
        label.widthAnchor.constraint(equalToConstant: 110).isActive = true

        let well = NSColorWell()
        well.color = FluidBackgroundSettings.color(from: hex)
        well.target = self
        well.action = action

        row.addArrangedSubview(label)
        row.addArrangedSubview(well)
        return (row, well)
    }

    private func sliderRow(
        title: String,
        value: Double,
        min: Double,
        max: Double,
        action: Selector
    ) -> (view: NSView, slider: NSSlider, valueLabel: NSTextField) {
        let row = NSStackView()
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 8

        let label = NSTextField(labelWithString: title)
        label.font = .systemFont(ofSize: 12, weight: .bold)
        label.textColor = Self.starleeWhite
        label.widthAnchor.constraint(equalToConstant: 110).isActive = true

        let slider = NSSlider(value: value, minValue: min, maxValue: max, target: self, action: action)
        slider.widthAnchor.constraint(equalToConstant: 190).isActive = true

        let valueLabel = NSTextField(labelWithString: formattedFluidValue(value))
        valueLabel.font = .monospacedDigitSystemFont(ofSize: 12, weight: .bold)
        valueLabel.textColor = Self.starleeCream
        valueLabel.widthAnchor.constraint(equalToConstant: 48).isActive = true

        row.addArrangedSubview(label)
        row.addArrangedSubview(slider)
        row.addArrangedSubview(valueLabel)
        return (row, slider, valueLabel)
    }

    private func formattedFluidValue(_ value: Double) -> String {
        if value.rounded() == value {
            return String(Int(value))
        }
        return String(format: "%.3f", value)
            .replacingOccurrences(of: #"0+$"#, with: "", options: .regularExpression)
            .replacingOccurrences(of: #"\.$"#, with: "", options: .regularExpression)
    }

    private func settingsCard(title: String, status: String, detail: String, actionTitle: String?, action: Selector?) -> NSView {
        let box = NSBox()
        box.boxType = .custom
        box.cornerRadius = 8
        box.borderWidth = 2
        box.borderColor = Self.starleeWhite.withAlphaComponent(0.44)
        box.fillColor = Self.starleeNavy.withAlphaComponent(0.85)
        box.translatesAutoresizingMaskIntoConstraints = false
        box.wantsLayer = true
        box.layer?.shadowColor = Self.starleeBlack.cgColor
        box.layer?.shadowOpacity = 0.28
        box.layer?.shadowRadius = 14
        box.layer?.shadowOffset = NSSize(width: 0, height: -8)
        box.widthAnchor.constraint(greaterThanOrEqualToConstant: 560).isActive = true

        let stack = NSStackView()
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 18
        stack.edgeInsets = NSEdgeInsets(top: 18, left: 20, bottom: 18, right: 20)
        stack.translatesAutoresizingMaskIntoConstraints = false
        box.addSubview(stack)

        let text = NSStackView()
        text.orientation = .vertical
        text.spacing = 4
        let titleLabel = NSTextField(labelWithString: title)
        titleLabel.font = .systemFont(ofSize: 18, weight: .heavy)
        titleLabel.textColor = Self.starleeWhite
        let detailLabel = NSTextField(wrappingLabelWithString: detail.isEmpty ? "No detail available." : detail)
        detailLabel.font = .systemFont(ofSize: 12, weight: .semibold)
        detailLabel.textColor = Self.starleeCream
        text.addArrangedSubview(titleLabel)
        text.addArrangedSubview(detailLabel)

        let statusLabel = NSTextField(labelWithString: status)
        statusLabel.font = .systemFont(ofSize: 12, weight: .heavy)
        statusLabel.textColor = statusColor(status)

        stack.addArrangedSubview(text)
        stack.addArrangedSubview(NSView())
        stack.addArrangedSubview(statusLabel)
        if let actionTitle, let action {
            let button = NSButton(title: actionTitle, target: self, action: action)
            styleSettingsActionButton(button)
            stack.addArrangedSubview(button)
        }

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: box.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: box.trailingAnchor),
            stack.topAnchor.constraint(equalTo: box.topAnchor),
            stack.bottomAnchor.constraint(equalTo: box.bottomAnchor)
        ])
        return box
    }

    private func reload() {
        guard isReloading == false else { return }
        isReloading = true
        progress.startAnimation(nil)
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            let doctor = self.client.runJSON(["doctor"])
            let recent = self.client.runJSONArray(["recent", "--limit", "500"]) ?? []
            let captures = recent.map(Self.capture(from:))
            DispatchQueue.main.async {
                self.doctor = doctor
                self.captures = captures
                self.groups = Self.monthGroups(from: captures)
                if self.selectedMonthID == nil || self.groups.contains(where: { $0.id == self.selectedMonthID }) == false {
                    self.selectedMonthID = self.groups.first?.id
                }
                self.progress.stopAnimation(nil)
                self.isReloading = false
                self.rebuildMonthButtons()
                self.render()
            }
        }
    }

    private func startAutomaticRefresh() {
        automaticRefreshTimer?.invalidate()
        let timer = Timer(timeInterval: 20, repeats: true) { [weak self] _ in
            guard self?.window?.isVisible == true else { return }
            self?.reload()
        }
        automaticRefreshTimer = timer
        RunLoop.main.add(timer, forMode: .common)
    }

    private static func capture(from value: [String: Any]) -> LibraryCapture {
        let title = value["title"] as? String ?? "Untitled"
        let urlString = value["url"] as? String
        let dateText = value["consumed_at"] as? String ?? value["captured_at"] as? String ?? ""
        return LibraryCapture(
            id: value["id"] as? String ?? title,
            title: title,
            type: value["type"] as? String ?? "note",
            site: value["site"] as? String,
            author: (value["author"] as? String).flatMap { $0.isEmpty ? nil : $0 },
            url: urlString.flatMap(URL.init(string:)),
            capturedAt: parseDate(dateText),
            capturedAtText: dateText,
            filePath: value["file_path"] as? String ?? "",
            snippet: value["snippet"] as? String ?? "",
            topics: (value["topics"] as? [String]) ?? []
        )
    }

    private static func monthGroups(from captures: [LibraryCapture]) -> [MonthGroup] {
        let grouped = Dictionary(grouping: captures, by: \.monthKey)
        return grouped.map { key, captures in
            MonthGroup(
                id: key,
                label: captures.first?.monthLabel ?? "Undated",
                captures: captures.sorted { ($0.capturedAt ?? .distantPast) > ($1.capturedAt ?? .distantPast) }
            )
        }
        .sorted { lhs, rhs in
            if lhs.id == "undated" { return false }
            if rhs.id == "undated" { return true }
            return lhs.id > rhs.id
        }
    }

    private static func parseDate(_ value: String) -> Date? {
        guard !value.isEmpty else { return nil }
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = iso.date(from: value) { return date }
        iso.formatOptions = [.withInternetDateTime]
        return iso.date(from: value)
    }

    private func rebuildMonthButtons() {
        monthStack.arrangedSubviews.forEach { view in
            monthStack.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        monthButtons.removeAll()
        sidebarDivider?.isHidden = groups.isEmpty
        if groups.isEmpty {
            refreshSidebarHoles()
            return
        }
        for group in groups {
            let button = SidebarBoxButton(title: group.label)
            button.target = self
            button.action = #selector(selectMonth(_:))
            button.identifier = NSUserInterfaceItemIdentifier(group.id)
            monthButtons[group.id] = button
            monthStack.addArrangedSubview(button)
        }
        refreshSidebarHoles()
    }

    private func updateSidebarSelection() {
        libraryButton.state = primaryView == .library ? .on : .off
        settingsButton.state = primaryView == .settings ? .on : .off
        libraryButton.setSelected(primaryView == .library)
        settingsButton.setSelected(primaryView == .settings)
        for (id, button) in monthButtons {
            let isSelected = primaryView == .library && id == selectedMonthID
            button.state = isSelected ? .on : .off
            (button as? SidebarBoxButton)?.setSelected(isSelected)
            button.isEnabled = true
        }
    }

    private func applyFilters() {
        let monthCaptures = groups.first { $0.id == selectedMonthID }?.captures ?? captures
        let query = searchField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if query.isEmpty {
            filteredCaptures = monthCaptures
        } else {
            filteredCaptures = monthCaptures.filter { capture in
                [capture.title, capture.source, capture.type, capture.snippet]
                    .joined(separator: " ")
                    .lowercased()
                    .contains(query)
            }
        }
        tableView.reloadData()
    }

    private func libraryPayloadJSON() -> String {
        let monthCaptures = groups.first { $0.id == selectedMonthID }?.captures ?? captures
        let ready = doctor?["ok"] as? Bool == true
        let readiness = ready ? "Ready" : "Needs setup"
        let monthLabel = groups.first { $0.id == selectedMonthID }?.label ?? "All captures"
        let payload: [String: Any] = [
            "monthLabel": monthLabel,
            "totalCount": captures.count,
            "readiness": readiness,
            "showOnboarding": !UserDefaults.standard.bool(forKey: Self.onboardingCompleteKey),
            "backgroundSettings": fluidBackground.webPayload,
            "captures": monthCaptures.map { capture in
                [
                    "id": capture.id,
                    "title": capture.title,
                    "type": capture.type,
                    "source": capture.source,
                    "author": capture.author ?? "",
                    "date": displayDate(capture.capturedAt, fallback: capture.capturedAtText),
                    "capturedAt": capture.capturedAtText,
                    "snippet": capture.snippet,
                    "url": capture.url?.absoluteString ?? "",
                    "filePath": capture.filePath,
                    "topics": capture.topics
                ]
            }
        ]
        guard
            JSONSerialization.isValidJSONObject(payload),
            let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
            let json = String(data: data, encoding: .utf8)
        else {
            return #"{"monthLabel":"Library","totalCount":0,"readiness":"Ready","captures":[]}"#
        }
        return json
    }

    private func addColumn(_ identifier: String, _ title: String, width: CGFloat) {
        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier(identifier))
        column.title = title
        column.width = width
        tableView.addTableColumn(column)
    }

    func numberOfRows(in tableView: NSTableView) -> Int {
        filteredCaptures.count
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard row < filteredCaptures.count else { return nil }
        let capture = filteredCaptures[row]
        let identifier = tableColumn?.identifier.rawValue ?? "title"
        let value: String
        switch identifier {
        case "title": value = capture.title
        case "source": value = capture.source
        case "type": value = displayType(capture.type)
        case "captured": value = displayDate(capture.capturedAt, fallback: capture.capturedAtText)
        case "transcript": value = capture.transcriptStatus
        case "file": value = URL(fileURLWithPath: capture.filePath).lastPathComponent
        default: value = ""
        }

        let cell = NSTableCellView()
        let field = NSTextField(labelWithString: value)
        field.lineBreakMode = .byTruncatingTail
        field.font = .systemFont(ofSize: identifier == "title" ? 13 : 12)
        field.textColor = identifier == "title" ? .labelColor : .secondaryLabelColor
        field.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(field)
        NSLayoutConstraint.activate([
            field.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
            field.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -6),
            field.centerYAnchor.constraint(equalTo: cell.centerYAnchor)
        ])
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        updateActionButtons()
    }

    func controlTextDidChange(_ obj: Notification) {
        applyFilters()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        if webView == appBackgroundWebView {
            updateFluidBackgroundRenderers()
            return
        }
        if webView == settingsWebView {
            settingsWebViewLoaded = true
            let payload = pendingSettingsPayload ?? settingsPayloadJSON()
            pendingSettingsPayload = nil
            webView.evaluateJavaScript("window.__starleeSettingsPayload = \(payload); if (window.renderStarleeSettings) { window.renderStarleeSettings(window.__starleeSettingsPayload); }", completionHandler: nil)
            updateFluidBackgroundRenderers()
            return
        }
        libraryWebViewLoaded = true
        // WKWebView :hover states only update while the host window delivers
        // mouse-moved events; without this the card hover border lagged or only
        // appeared after a click (REQ-001).
        webView.window?.acceptsMouseMovedEvents = true
        let payload = pendingLibraryPayload ?? libraryPayloadJSON()
        pendingLibraryPayload = nil
        webView.evaluateJavaScript("window.__starleeLibraryPayload = \(payload); if (window.renderStarleeLibrary) { window.renderStarleeLibrary(window.__starleeLibraryPayload); }", completionHandler: nil)
        updateFluidBackgroundRenderers()
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard
            message.name == "starlee",
            let body = message.body as? [String: Any],
            let action = body["action"] as? String
        else { return }
        switch action {
        case "refresh":
            refresh()
        case "setBackground":
            if let settings = body["settings"] as? [String: Any] {
                fluidBackground = FluidBackgroundSettings(payload: settings)
                saveAndApplyFluidBackground()
            }
        case "openVault":
            menuController?.openVault()
        case "openBrowserSetup":
            menuController?.browserSetup()
        case "codexGuide":
            showCodexGuide()
        case "copyDiagnostics":
            copySupportBundle()
        case "import":
            importDocument()
        case "open":
            if let id = body["id"] as? String { presentReader(id: id) }
        case "delete":
            if let id = body["id"] as? String {
                confirmAndDelete(id: id, title: (body["title"] as? String) ?? "this capture")
            }
        case "openURL":
            if let urlString = body["url"] as? String, let url = URL(string: urlString) {
                NSWorkspace.shared.open(url)
            }
        case "reveal":
            if let path = body["path"] as? String, !path.isEmpty {
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
            }
        case "setTopics":
            if let id = body["id"] as? String {
                setRecordTopics(id: id, topics: (body["topics"] as? [String]) ?? [])
            }
        case "upload":
            uploadDocuments()
        case "onboardingDone":
            UserDefaults.standard.set(true, forKey: Self.onboardingCompleteKey)
        case "exportBrain":
            exportBrain()
        case "ingestBrain":
            ingestBrain()
        case "rerunOnboarding":
            rerunOnboarding()
        default:
            break
        }
    }

    /// Export an audited, shareable copy of the vault. Restricted bodies are
    /// stripped by the CLI before the bundle is written.
    private func exportBrain() {
        guard let window else { return }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "my-brain.starlee"
        panel.message = "Export a shareable copy of your brain. Restricted article bodies are always removed."
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url, let self else { return }
            self.client.runAsync(["export", url.path]) { _ in
                DialogPresenter.show(
                    title: "Brain exported",
                    message: "Saved \(url.lastPathComponent). Restricted article bodies were excluded from the bundle."
                )
            }
        }
    }

    /// Mount a friend's `.starlee` bundle read-only so it can be searched with
    /// scope: borrowed.
    private func ingestBrain() {
        guard let window else { return }
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.message = "Open a .starlee bundle to search it read-only."
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url, let self else { return }
            self.client.runAsync(["ingest", url.path]) { [weak self] _ in
                self?.reload()
                DialogPresenter.show(
                    title: "Brain added",
                    message: "You can now search \(url.lastPathComponent) from Codex with scope: borrowed."
                )
            }
        }
    }

    /// Replay the first-run onboarding from Settings.
    private func rerunOnboarding() {
        UserDefaults.standard.set(false, forKey: Self.onboardingCompleteKey)
        showLibrary()
        libraryWebView?.evaluateJavaScript(
            "if (window.showStarleeOnboarding) { window.showStarleeOnboarding(); }",
            completionHandler: nil
        )
    }

    /// Bulk-import user documents (PDF, Word, text, Markdown) with an optional
    /// shared topic, then reload the Library. Parsing happens locally in the CLI.
    private func uploadDocuments() {
        guard let window else { return }
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        var types: [UTType] = [.pdf, .plainText, .text, .utf8PlainText]
        for ext in ["docx", "md", "markdown"] {
            if let type = UTType(filenameExtension: ext) { types.append(type) }
        }
        panel.allowedContentTypes = types
        panel.message = "Choose PDF, Word, text, or Markdown files to add to your brain."
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let self, !panel.urls.isEmpty else { return }
            let urls = panel.urls
            let topic = self.promptForBatchTopic(count: urls.count)
            guard topic != nil else { return } // user cancelled the topic step
            var arguments = ["import"]
            arguments.append(contentsOf: urls.map { $0.path })
            if let topic, !topic.isEmpty {
                arguments.append("--topic")
                arguments.append(topic)
            }
            self.progress.startAnimation(nil)
            self.client.runAsync(arguments) { [weak self] output in
                guard let self else { return }
                self.reportImport(output: output, total: urls.count)
                self.reload()
            }
        }
    }

    /// Returns the chosen topic (possibly empty for "no topic"), or nil if the
    /// user cancelled the import entirely.
    private func promptForBatchTopic(count: Int) -> String? {
        let alert = NSAlert()
        alert.messageText = "Add a topic to \(count) document\(count == 1 ? "" : "s")?"
        alert.informativeText = "Optional — leave blank to import without a topic."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 240, height: 24))
        field.placeholderString = "Topic (optional)"
        alert.accessoryView = field
        alert.addButton(withTitle: "Import")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return nil }
        return field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func reportImport(output: String, total: Int) {
        guard
            let data = output.data(using: .utf8),
            let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return }
        let imported = (value["imported"] as? [[String: Any]])?.count ?? 0
        let skipped = value["skipped"] as? [[String: Any]] ?? []
        guard !skipped.isEmpty else { return } // silent on full success; reload shows results
        let reasons = skipped.prefix(5).compactMap { entry -> String? in
            guard let path = entry["path"] as? String else { return nil }
            let name = URL(fileURLWithPath: path).lastPathComponent
            let reason = entry["reason"] as? String ?? "could not be read"
            return "• \(name): \(reason)"
        }
        DialogPresenter.show(
            title: "Imported \(imported) of \(total)",
            message: "Some files were skipped:\n\n" + reasons.joined(separator: "\n")
        )
    }

    /// Persist a record's topic set, then refresh the Library so cards and the
    /// filter facets reflect the change. The reader updates its chips
    /// optimistically, so it is left open and untouched.
    private func setRecordTopics(id: String, topics: [String]) {
        var arguments = ["set-topics", id]
        for topic in topics {
            arguments.append("--topic")
            arguments.append(topic)
        }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            _ = self.client.run(arguments)
            DispatchQueue.main.async { self.reload() }
        }
    }

    /// Fetch the full record (metadata incl. topics + body) off the main thread,
    /// then hand it to the renderer's reader overlay.
    private func presentReader(id: String) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            let result = self.client.runJSON(["get", id])
            DispatchQueue.main.async {
                let payload = self.readerPayloadJSON(id: id, result: result)
                self.libraryWebView?.evaluateJavaScript(
                    "if (window.renderStarleeReader) { window.renderStarleeReader(\(payload)); }",
                    completionHandler: nil
                )
            }
        }
    }

    private func readerPayloadJSON(id: String, result: [String: Any]?) -> String {
        let capture = captures.first { $0.id == id }
        guard
            let record = result?["record"] as? [String: Any],
            let metadata = record["metadata"] as? [String: Any]
        else {
            return jsonObjectString(["id": id, "error": "This record is no longer available."])
        }
        // Display layer is metadata-only: the captured body/transcript is never
        // sent to the UI. It stays on disk in the vault for `starlee search` and
        // `starlee_query`; the reader only shows attribution and a source link.
        var reader: [String: Any] = [
            "id": id,
            "title": (metadata["title"] as? String) ?? capture?.title ?? "Untitled",
            "type": (metadata["type"] as? String) ?? capture?.type ?? "note",
            "filePath": (record["file_path"] as? String) ?? capture?.filePath ?? "",
        ]
        if let url = metadata["url"] as? String, !url.isEmpty {
            reader["url"] = url
        } else if let url = capture?.url?.absoluteString {
            reader["url"] = url
        }
        if let author = metadata["author"] as? String, !author.isEmpty {
            reader["author"] = author
        }
        if let published = metadata["published_at"] as? String, !published.isEmpty {
            reader["publishedAt"] = published
        }
        if let capture {
            reader["date"] = displayDate(capture.capturedAt, fallback: capture.capturedAtText)
        }
        if let topics = metadata["topics"] as? [String], !topics.isEmpty {
            reader["topics"] = topics
        }
        return jsonObjectString(reader)
    }

    /// Confirm the permanent delete natively, then remove the capture and reload.
    private func confirmAndDelete(id: String, title: String) {
        let alert = NSAlert()
        alert.messageText = "Delete “\(title)” permanently?"
        alert.informativeText =
            "This removes the capture, its text or transcript, and all of its metadata from your brain and your local index. This cannot be undone."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Delete")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            _ = self.client.run(["delete", id])
            DispatchQueue.main.async {
                self.libraryWebView?.evaluateJavaScript(
                    "if (window.closeStarleeReader) { window.closeStarleeReader(); }",
                    completionHandler: nil
                )
                self.reload()
            }
        }
    }

    private func jsonObjectString(_ object: [String: Any]) -> String {
        guard
            JSONSerialization.isValidJSONObject(object),
            let data = try? JSONSerialization.data(withJSONObject: object, options: []),
            let json = String(data: data, encoding: .utf8)
        else {
            return #"{"error":"Could not render this capture."}"#
        }
        return json
    }

    private func updateActionButtons() {
        let capture = selectedCapture()
        openButton.isEnabled = capture?.url != nil
        revealButton.isEnabled = capture?.filePath.isEmpty == false
    }

    private func selectedCapture() -> LibraryCapture? {
        let row = tableView.selectedRow
        guard row >= 0, row < filteredCaptures.count else { return nil }
        return filteredCaptures[row]
    }

    private func displayType(_ value: String) -> String {
        switch value {
        case "youtube": return "YouTube"
        case "spotify_episode": return "Spotify"
        case "article": return "Article"
        case "note": return "Note"
        default: return value.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }

    private func displayDate(_ date: Date?, fallback: String) -> String {
        guard let date else { return fallback.isEmpty ? "Undated" : fallback }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .none
        return formatter.string(from: date)
    }

    private func status() -> [String: Any] {
        doctor?["status"] as? [String: Any] ?? [:]
    }

    private func statusString(_ key: String) -> String {
        let value = status()[key]
        if let string = value as? String { return string }
        if let number = value as? NSNumber { return number.stringValue }
        return "unknown"
    }

    private func browserSetupStatus(_ setup: [String: Any], bridge: [String: Any]) -> String {
        switch setup["state"] as? String {
        case "capture_test_passed":
            return "Capture test passed"
        case "capture_test_needed":
            return "Test needed"
        case "permission_needed":
            return "Permission needed"
        case "check_in_needed":
            return "Reload extension"
        case "install_needed":
            return "Install needed"
        default:
            return (bridge["ok"] as? Bool) == true ? "Ready" : "Needs attention"
        }
    }

    private func browserSetupDetail(_ setup: [String: Any], bridge: [String: Any]) -> String {
        var lines: [String] = []
        if let detail = setup["detail"] as? String {
            lines.append(detail)
        }
        if let next = setup["next_action"] as? String ?? bridge["recommended_next_action"] as? String {
            lines.append("Next: \(next)")
        }
        if let version = bridge["extension_version"] as? String {
            let build = bridge["extension_build"] as? String ?? "unknown"
            lines.append("Chrome extension \(version) (\(build))")
        }
        if let passedAt = setup["capture_test_passed_at"] as? String {
            lines.append("Capture test: \(passedAt)")
        }
        if lines.isEmpty {
            return "Chrome uses the local extension folder. Safari uses the Starlee Capture extension wrapper."
        }
        return lines.joined(separator: "\n")
    }

    private func checksByName() -> [String: (ok: Bool, detail: String)] {
        let checks = doctor?["checks"] as? [[String: Any]] ?? []
        return Dictionary(uniqueKeysWithValues: checks.map {
            (
                $0["name"] as? String ?? "unknown",
                (
                    ok: $0["ok"] as? Bool ?? false,
                    detail: $0["detail"] as? String ?? ""
                )
            )
        })
    }

    private func redactedSupportBundle() -> String {
        let checks = checksByName()
            .sorted { $0.key < $1.key }
            .map { "- \($0.key): \($0.value.ok ? "ok" : "needs_action") - \($0.value.detail)" }
            .joined(separator: "\n")
        let nextActions = ((doctor?["next_actions"] as? [String]) ?? [])
            .map { "- \($0)" }
            .joined(separator: "\n")
        return """
        Starlee redacted diagnostics
        Overall: \(doctor?["ok"] as? Bool == true ? "ok" : "needs_attention")
        Home: \(statusString("home"))
        Vault: \(statusString("vault"))
        Index: \(statusString("index"))
        Capture endpoint: \(statusString("capture_endpoint"))
        Capture token path: redacted

        Checks:
        \(checks.isEmpty ? "- none" : checks)

        Next actions:
        \(nextActions.isEmpty ? "- none" : nextActions)
        """
    }

    private func statusColor(_ status: String) -> NSColor {
        let lower = status.lowercased()
        if lower.contains("ready") || lower.contains("installed") || lower.contains("local") {
            return Self.starleeWhite
        }
        if lower.contains("needs") || lower.contains("missing") {
            return Self.starleeCream
        }
        return Self.starleeCream
    }

    private func styleSettingsActionButton(_ button: NSButton) {
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.wantsLayer = true
        button.layer?.backgroundColor = Self.starleeWhite.cgColor
        button.layer?.borderColor = Self.starleeBlack.cgColor
        button.layer?.borderWidth = 2
        button.layer?.cornerRadius = 0
        button.contentTintColor = Self.starleeBlack
        button.font = .systemFont(ofSize: 12, weight: .bold)
        button.heightAnchor.constraint(equalToConstant: 26).isActive = true
        button.widthAnchor.constraint(greaterThanOrEqualToConstant: 74).isActive = true
        button.attributedTitle = NSAttributedString(
            string: button.title,
            attributes: [
                .foregroundColor: Self.starleeBlack,
                .font: NSFont.systemFont(ofSize: 12, weight: .bold)
            ]
        )
    }

    private func applyFluidBackground() {
        window?.appearance = NSAppearance(named: .aqua)
        rootSplitView?.layer?.backgroundColor = NSColor.clear.cgColor
        tableView.backgroundColor = NSColor.controlBackgroundColor.withAlphaComponent(0.72)
        tableView.enclosingScrollView?.backgroundColor = tableView.backgroundColor
        updateFluidBackgroundControls()
        updateFluidBackgroundRenderers()
    }

    private func saveAndApplyFluidBackground() {
        fluidBackgroundStore.save(fluidBackground)
        applyFluidBackground()
    }

    private func updateFluidBackgroundRenderers() {
        let script = "if (window.applyStarleeBackgroundSettings) { window.applyStarleeBackgroundSettings(\(fluidBackground.webPayloadJSON)); }"
        appBackgroundWebView?.evaluateJavaScript(script, completionHandler: nil)
        libraryWebView?.evaluateJavaScript(script, completionHandler: nil)
        settingsWebView?.evaluateJavaScript(script, completionHandler: nil)
    }

    private func updateFluidBackgroundControls() {
        pixelColorWell?.color = FluidBackgroundSettings.color(from: fluidBackground.pixelColor)
        backgroundColorWell?.color = FluidBackgroundSettings.color(from: fluidBackground.backgroundColor)
        blackColorWell?.color = FluidBackgroundSettings.color(from: fluidBackground.black)
        whiteColorWell?.color = FluidBackgroundSettings.color(from: fluidBackground.white)
        pixelSizeSlider?.doubleValue = fluidBackground.pixelSize
        thresholdSlider?.doubleValue = fluidBackground.threshold
        fluidSpeedSlider?.doubleValue = fluidBackground.speed
        zoomSlider?.doubleValue = fluidBackground.zoom
        pixelSizeValueLabel?.stringValue = formattedFluidValue(fluidBackground.pixelSize)
        thresholdValueLabel?.stringValue = formattedFluidValue(fluidBackground.threshold)
        fluidSpeedValueLabel?.stringValue = formattedFluidValue(fluidBackground.speed)
        zoomValueLabel?.stringValue = formattedFluidValue(fluidBackground.zoom)
    }

    @objc private func showLibrary() {
        primaryView = .library
        render()
    }

    @objc private func showSettings() {
        primaryView = .settings
        render()
    }

    @objc private func selectMonth(_ sender: NSButton) {
        selectedMonthID = sender.identifier?.rawValue
        primaryView = .library
        render()
    }

    @objc private func refresh() {
        reload()
    }

    @objc private func changePixelColor(_ sender: NSColorWell) {
        fluidBackground.pixelColor = FluidBackgroundSettings.hex(from: sender.color)
        saveAndApplyFluidBackground()
    }

    @objc private func changeBackgroundColor(_ sender: NSColorWell) {
        fluidBackground.backgroundColor = FluidBackgroundSettings.hex(from: sender.color)
        saveAndApplyFluidBackground()
    }

    @objc private func changePixelSize(_ sender: NSSlider) {
        fluidBackground.pixelSize = sender.doubleValue.rounded()
        saveAndApplyFluidBackground()
    }

    @objc private func changeThreshold(_ sender: NSSlider) {
        fluidBackground.threshold = sender.doubleValue
        saveAndApplyFluidBackground()
    }

    @objc private func changeFluidSpeed(_ sender: NSSlider) {
        fluidBackground.speed = sender.doubleValue
        saveAndApplyFluidBackground()
    }

    @objc private func changeZoom(_ sender: NSSlider) {
        fluidBackground.zoom = sender.doubleValue
        saveAndApplyFluidBackground()
    }

    @objc private func selectFluidLook(_ sender: NSButton) {
        guard
            let name = sender.identifier?.rawValue,
            let look = FluidBackgroundLooks.all.first(where: { $0.name == name })
        else { return }
        fluidBackground = look.settings
        // Procedural engines get a fresh seed so the field looks new every pick.
        if fluidBackground.kind != "pixel-dither" {
            fluidBackground.flowSeed = Double.random(in: 0...1)
        }
        saveAndApplyFluidBackground()
        // Switching engines swaps which fine-tuning controls apply; rebuild the
        // panel so it matches. Same-engine picks also refresh the selection.
        if primaryView == .settings {
            render()
        }
    }

    @objc private func selectFlowFinish(_ sender: NSSegmentedControl) {
        let finishes = ["sharp", "soft", "glass"]
        let index = max(0, min(finishes.count - 1, sender.selectedSegment))
        fluidBackground.flowFinish = finishes[index]
        saveAndApplyFluidBackground()
    }

    @objc private func randomizeSeed() {
        fluidBackground.flowSeed = Double.random(in: 0...1)
        saveAndApplyFluidBackground()
    }

    @objc private func changeBlackColor(_ sender: NSColorWell) {
        fluidBackground.black = FluidBackgroundSettings.hex(from: sender.color)
        saveAndApplyFluidBackground()
    }

    @objc private func changeWhiteColor(_ sender: NSColorWell) {
        fluidBackground.white = FluidBackgroundSettings.hex(from: sender.color)
        saveAndApplyFluidBackground()
    }

    @objc private func changeAuroraIntensity(_ sender: NSSlider) {
        fluidBackground.auroraIntensity = sender.doubleValue
        setRowValueLabel(sender, formattedFluidValue(sender.doubleValue))
        saveAndApplyFluidBackground()
    }

    @objc private func changeDitherDotSize(_ sender: NSSlider) {
        let v = sender.doubleValue.rounded()
        fluidBackground.ditherDotSize = v
        setRowValueLabel(sender, formattedFluidValue(v))
        saveAndApplyFluidBackground()
    }

    @objc private func changeDitherContrast(_ sender: NSSlider) {
        fluidBackground.ditherContrast = sender.doubleValue
        setRowValueLabel(sender, formattedFluidValue(sender.doubleValue))
        saveAndApplyFluidBackground()
    }

    @objc private func changeDitherNavyBuffer(_ sender: NSSlider) {
        fluidBackground.ditherNavyBuffer = sender.doubleValue
        setRowValueLabel(sender, formattedFluidValue(sender.doubleValue))
        saveAndApplyFluidBackground()
    }

    @objc private func selectGlassMode(_ sender: NSSegmentedControl) {
        fluidBackground.glassMode = sender.selectedSegment == 1 ? "blur" : "panes"
        saveAndApplyFluidBackground()
        // Panes/Refraction controls only apply in panes mode; rebuild to match.
        if primaryView == .settings { render() }
    }

    @objc private func changeGlassPanes(_ sender: NSSlider) {
        let v = sender.doubleValue.rounded()
        fluidBackground.glassPanes = v
        setRowValueLabel(sender, formattedFluidValue(v))
        saveAndApplyFluidBackground()
    }

    @objc private func changeGlassSoftness(_ sender: NSSlider) {
        let v = sender.doubleValue.rounded()
        fluidBackground.glassSoftness = v
        setRowValueLabel(sender, formattedFluidValue(v))
        saveAndApplyFluidBackground()
    }

    @objc private func changeGlassBrightness(_ sender: NSSlider) {
        fluidBackground.glassBrightness = sender.doubleValue
        setRowValueLabel(sender, formattedFluidValue(sender.doubleValue))
        saveAndApplyFluidBackground()
    }

    @objc private func changeGlassRefraction(_ sender: NSSlider) {
        fluidBackground.glassRefraction = sender.doubleValue
        setRowValueLabel(sender, formattedFluidValue(sender.doubleValue))
        saveAndApplyFluidBackground()
    }

    @objc private func openVault() {
        menuController?.openVault()
    }

    @objc private func openBrowserSetup() {
        menuController?.browserSetup()
    }

    @objc private func showCodexGuide() {
        DialogPresenter.show(
            title: "Codex plugin",
            message: """
            The Starlee Codex plugin lets Codex query your local captures through local MCP tools.

            To install or repair it, run:

            ./scripts/install.sh
            """
        )
    }

    @objc private func copySupportBundle() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(redactedSupportBundle(), forType: .string)
        DialogPresenter.show(title: "Copied Diagnostics", message: "A redacted support bundle was copied to the clipboard.")
    }

    @objc private func importDocument() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [.plainText, .text, .utf8PlainText]
        panel.beginSheetModal(for: window!) { [weak self] response in
            guard response == .OK, let url = panel.url, let self else { return }
            guard let body = try? String(contentsOf: url, encoding: .utf8) else {
                DialogPresenter.show(title: "Import failed", message: "Starlee can import UTF-8 text and Markdown files from the desktop app.")
                return
            }
            let title = url.deletingPathExtension().lastPathComponent
            self.client.runAsync(["capture-text", "--title", title, "--text", body, "--type", "note"]) { _ in
                self.reload()
            }
        }
    }

    @objc private func openSelectedCapture() {
        guard let url = selectedCapture()?.url else { return }
        NSWorkspace.shared.open(url)
    }

    @objc private func revealSelectedCapture() {
        guard let path = selectedCapture()?.filePath, !path.isEmpty else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }
}

/// Sidebar nav plaque. Shares the Library article card surface system
/// (`.capture-card` in renderer/styles.css, mirrored natively in
/// `settingsCard`/`appearancePanel`): translucent navy fill so the solid-black
/// sidebar shows through ~15% for subtle depth, a light card border that
/// brightens on hover/selection, an 8pt corner radius, and a card-style drop
/// shadow — formatted as a bold, outlined nav button. It does not bake any
/// background image into the surface.
private final class SidebarBoxButton: NSButton {
    static var labelFont: NSFont {
        NSFont(name: "Avenir Next Condensed Heavy", size: 26)
            ?? NSFont(name: "Avenir Next Heavy", size: 24)
            ?? NSFont(name: "Helvetica Neue Condensed Black", size: 24)
            ?? .systemFont(ofSize: 24, weight: .heavy)
    }

    // Surface tokens mirror the Library article cards (.capture-card):
    // navy #13284B (rgba(19,40,75,…)), light border, 8pt radius, card shadow.
    // Exact card token: CSS `rgba(19, 40, 75, …)` in sRGB. The article cards are
    // rendered by WebKit in sRGB, so the fill must use sRGB here too —
    // calibratedRGB with the same numbers displays as a noticeably different tint.
    private static let navy = NSColor(srgbRed: 19.0 / 255.0, green: 40.0 / 255.0, blue: 75.0 / 255.0, alpha: 1)
    private static let cream = NSColor(srgbRed: 0.949, green: 0.890, blue: 0.714, alpha: 1)
    static let cornerRadius: CGFloat = 8
    private static let buttonHeight: CGFloat = 84
    private static let plaqueInsetX: CGFloat = 6
    private static let plaqueInsetY: CGFloat = 8

    /// The plaque rectangle within the button's bounds. The sidebar cuts a hole
    /// of exactly this shape so the app background shows through the translucent
    /// navy fill; the plaque does not shift on hover/press so it stays aligned
    /// with that hole.
    var restingPlaqueRect: NSRect {
        bounds.insetBy(dx: Self.plaqueInsetX, dy: Self.plaqueInsetY)
    }

    private var trackingAreaRef: NSTrackingArea?
    private var isHovering = false
    private var isPressed = false
    private var isSelected = false

    init(title: String) {
        super.init(frame: .zero)
        self.title = title
        isBordered = false
        bezelStyle = .regularSquare
        setButtonType(.momentaryChange)
        alignment = .center
        font = Self.labelFont
        contentTintColor = .white
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 260).isActive = true
        heightAnchor.constraint(equalToConstant: Self.buttonHeight).isActive = true
        updateAttributedTitle()
    }

    required init?(coder: NSCoder) {
        nil
    }

    override var title: String {
        didSet {
            updateAttributedTitle()
        }
    }

    override var isEnabled: Bool {
        didSet {
            alphaValue = 1
            updateAttributedTitle()
        }
    }

    override var acceptsFirstResponder: Bool {
        true
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingAreaRef {
            removeTrackingArea(trackingAreaRef)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        trackingAreaRef = area
    }

    override func mouseEntered(with event: NSEvent) {
        isHovering = true
        needsDisplay = true
    }

    override func mouseExited(with event: NSEvent) {
        isHovering = false
        needsDisplay = true
    }

    override func mouseDown(with event: NSEvent) {
        isPressed = true
        needsDisplay = true
        super.mouseDown(with: event)
        isPressed = false
        needsDisplay = true
    }

    override func becomeFirstResponder() -> Bool {
        needsDisplay = true
        return super.becomeFirstResponder()
    }

    override func resignFirstResponder() -> Bool {
        needsDisplay = true
        return super.resignFirstResponder()
    }

    func setSelected(_ selected: Bool) {
        guard isSelected != selected else { return }
        isSelected = selected
        needsDisplay = true
    }

    private func updateAttributedTitle() {
        attributedTitle = NSAttributedString(
            string: title,
            attributes: [
                .font: Self.labelFont,
                .foregroundColor: NSColor.white
            ]
        )
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        let active = isSelected
        // The plaque stays at its resting rect so it aligns with the hole the
        // sidebar cuts behind it; hover/press are expressed through fill, border,
        // and shadow only — never movement.
        let plaque = restingPlaqueRect
        let plaquePath = NSBezierPath(roundedRect: plaque, xRadius: Self.cornerRadius, yRadius: Self.cornerRadius)

        // Card-style drop shadow + translucent navy fill (the card token,
        // navy @ ~0.85). The button-shaped hole behind lets the live app
        // background show through the remaining ~15%, matching the article cards.
        NSGraphicsContext.saveGraphicsState()
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(isPressed ? 0.22 : (isHovering ? 0.40 : 0.32))
        shadow.shadowBlurRadius = isPressed ? 8 : (isHovering ? 22 : 16)
        shadow.shadowOffset = NSSize(width: 0, height: isPressed ? -3 : -8)
        shadow.set()
        let fillAlpha: CGFloat = isPressed ? 0.90 : ((isHovering || active) ? 0.92 : 0.85)
        Self.navy.withAlphaComponent(fillAlpha).setFill()
        plaquePath.fill()
        NSGraphicsContext.restoreGraphicsState()

        // Inner cream hairline = the retro double-border plaque treatment.
        let innerRect = plaque.insetBy(dx: 5, dy: 5)
        let innerRadius = max(Self.cornerRadius - 3, 2)
        let innerPath = NSBezierPath(roundedRect: innerRect, xRadius: innerRadius, yRadius: innerRadius)
        Self.cream.withAlphaComponent((isHovering || active) ? 0.92 : 0.78).setStroke()
        innerPath.lineWidth = 1
        innerPath.stroke()

        // Outer border mirrors the card border exactly: white @ 0.42, full white
        // on hover/selection.
        let borderAlpha: CGFloat = (isHovering || active) ? 1.0 : 0.42
        NSColor.white.withAlphaComponent(borderAlpha).setStroke()
        plaquePath.lineWidth = 2
        plaquePath.stroke()

        // Selected: subtle outer glow ring so the active item reads as selected
        // without becoming a different component.
        if active {
            NSColor.white.withAlphaComponent(0.30).setStroke()
            let glow = NSBezierPath(
                roundedRect: plaque.insetBy(dx: -3, dy: -3),
                xRadius: Self.cornerRadius + 3,
                yRadius: Self.cornerRadius + 3
            )
            glow.lineWidth = 2
            glow.stroke()
        }

        // Keyboard focus ring.
        if window?.firstResponder === self {
            Self.cream.withAlphaComponent(0.80).setStroke()
            let focusPath = NSBezierPath(
                roundedRect: plaque.insetBy(dx: -3, dy: -3),
                xRadius: Self.cornerRadius + 3,
                yRadius: Self.cornerRadius + 3
            )
            focusPath.lineWidth = 2
            focusPath.stroke()
        }

        drawCenteredTitle(in: plaque)
    }

    private func drawCenteredTitle(in rect: NSRect) {
        let text = title.uppercased()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: Self.labelFont,
            .foregroundColor: NSColor.white,
            .kern: 2.4,
            .shadow: textShadow
        ]
        let attributed = NSAttributedString(string: text, attributes: attributes)
        let textSize = attributed.size()
        let textRect = NSRect(
            x: rect.midX - textSize.width / 2,
            y: rect.midY - textSize.height / 2 + 2,
            width: textSize.width,
            height: textSize.height
        )
        attributed.draw(in: textRect)
    }

    private var textShadow: NSShadow {
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.58)
        shadow.shadowBlurRadius = 2.5
        shadow.shadowOffset = NSSize(width: 0, height: -1)
        return shadow
    }
}

/// A solid-black sidebar background that cuts button-shaped holes so the
/// app background WebView behind the split view shows through the nav plaques
/// only. The black field everywhere else stays fully opaque — the sidebar
/// container is never made transparent as a whole. Each button then draws its
/// translucent navy plaque over its hole, so the live, Settings-reactive
/// background faintly shows through at ~15%, matching the article cards.
private final class SidebarHoleBackgroundView: NSView {
    var knockoutButtons: [SidebarBoxButton] = [] {
        didSet { needsDisplay = true }
    }

    override var isFlipped: Bool { false }

    // Holes depend on the button frames, which Auto Layout resolves during the
    // layout pass; redraw whenever that geometry can have changed.
    override func layout() {
        super.layout()
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        // Fill the whole sidebar black, then subtract each plaque rect with the
        // even-odd rule. Unpainted holes are transparent, revealing the
        // background WebView that sits behind the split view.
        let path = NSBezierPath(rect: bounds)
        path.windingRule = .evenOdd
        for button in knockoutButtons where button.window === window && button.superview != nil {
            let rect = button.convert(button.restingPlaqueRect, to: self)
            path.append(
                NSBezierPath(
                    roundedRect: rect,
                    xRadius: SidebarBoxButton.cornerRadius,
                    yRadius: SidebarBoxButton.cornerRadius
                )
            )
        }
        NSColor.black.setFill()
        path.fill()
    }
}
