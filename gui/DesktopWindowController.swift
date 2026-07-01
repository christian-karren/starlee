import AppKit
import UniformTypeIdentifiers
import WebKit

// "wght" axis tag as UInt32 cast to Int for NSDictionary key
private let kInterWeightAxis: Int = 0x77676874

private enum SidebarScope: Hashable {
    case all
    case favorites
    case month(String)
    case topic(String)
    case source(String)
    case company(String)
    case country(String)
    case customEntity(String)
    case settings
}

private extension NSFont {
    /// Returns InterVariable at the requested size and weight, falling back to
    /// the system font if the bundled font isn't available yet.
    static func inter(ofSize size: CGFloat, weight: NSFont.Weight = .regular) -> NSFont {
        let w: Double
        switch weight {
        case .ultraLight: w = 100
        case .thin:       w = 200
        case .light:      w = 300
        case .medium:     w = 500
        case .semibold:   w = 600
        case .bold:       w = 700
        case .heavy:      w = 800
        case .black:      w = 900
        default:          w = 400
        }
        let desc = NSFontDescriptor(fontAttributes: [
            .name: "InterVariable",
            .variation: [kInterWeightAxis: w]
        ])
        return NSFont(descriptor: desc, size: size)
            ?? .systemFont(ofSize: size, weight: weight)
    }
}

private final class FavoritesStore {
    private let url: URL

    init(fileManager: FileManager = .default) {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory())
        let directory = base.appendingPathComponent("Starlee", isDirectory: true)
        try? fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        url = directory.appendingPathComponent("favorites.json")
    }

    func load() -> Set<String> {
        guard
            let data = try? Data(contentsOf: url),
            let value = try? JSONSerialization.jsonObject(with: data) as? [String]
        else { return [] }
        return Set(value.filter { !$0.isEmpty })
    }

    func save(_ ids: Set<String>) {
        guard
            let data = try? JSONSerialization.data(withJSONObject: Array(ids).sorted(), options: [.prettyPrinted])
        else { return }
        try? data.write(to: url, options: [.atomic])
    }
}

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
        let taxonomyTopics: [String]
        let companies: [String]
        let countries: [String]
        let customEntities: [String]
        let favorite: Bool

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

        var sourceKind: String {
            switch type {
            case "youtube", "spotify_episode":
                return "media"
            default:
                return "article"
            }
        }

        var sourceLabel: String {
            if sourceKind == "media", let author, !author.isEmpty {
                return Self.cleanMediaSourceLabel(author)
            }
            return Self.cleanArticleSourceLabel(source)
        }

        var sourceKey: String {
            "\(sourceKind):\(sourceLabel.lowercased())"
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

        private static func cleanArticleSourceLabel(_ value: String) -> String {
            let lower = value.lowercased()
                .replacingOccurrences(of: "www.", with: "")
                .replacingOccurrences(of: "m.", with: "")
            let known = [
                "stratechery.com": "Stratechery",
                "nytimes.com": "New York Times",
                "newyorktimes.com": "New York Times",
                "wsj.com": "Wall Street Journal",
                "washingtonpost.com": "Washington Post",
                "bloomberg.com": "Bloomberg",
                "theinformation.com": "The Information",
                "semianalysis.com": "SemiAnalysis",
                "substack.com": "Substack",
                "youtube.com": "YouTube"
            ]
            if let label = known[lower] { return label }
            let withoutTLD = lower
                .replacingOccurrences(of: ".com", with: "")
                .replacingOccurrences(of: ".org", with: "")
                .replacingOccurrences(of: ".net", with: "")
                .replacingOccurrences(of: ".io", with: "")
                .replacingOccurrences(of: ".co", with: "")
            return withoutTLD
                .split(separator: ".")
                .last
                .map { $0.split(separator: "-").map { $0.capitalized }.joined(separator: " ") }
                ?? value
        }

        private static func cleanMediaSourceLabel(_ value: String) -> String {
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            let withoutPrefix = trimmed.hasPrefix("The ") ? String(trimmed.dropFirst(4)) : trimmed
            if let colon = withoutPrefix.firstIndex(of: ":") {
                return String(withoutPrefix[..<colon]).trimmingCharacters(in: .whitespacesAndNewlines)
            }
            return withoutPrefix
        }
    }

    private struct MonthGroup {
        let id: String
        let label: String
        let captures: [LibraryCapture]
    }

    private struct NavNode {
        let id: String
        let label: String
        let count: Int?
        let scope: SidebarScope?
        let children: [NavNode]

        var hasChildren: Bool {
            !children.isEmpty
        }
    }

    private let client: StarleeClient
    private weak var menuController: StatusMenuController?
    private let fluidBackgroundStore = FluidBackgroundSettingsStore()
    private let menuBarSettingsStore = MenuBarSettingsStore()
    private var primaryView: PrimaryView = .library
    private var doctor: [String: Any]?
    private var captures: [LibraryCapture] = []
    private var groups: [MonthGroup] = []
    private var filteredCaptures: [LibraryCapture] = []
    private var selectedSidebarScope: SidebarScope = .all
    private var expandedSidebarNodeIDs: Set<String> = [
        "my-library",
        "topics",
        "time",
        "sources",
        "source-articles",
        "source-media",
        "companies",
        "countries",
        "custom-entities"
    ]
    private var favoriteIDs: Set<String> = []
    private let favoritesStore = FavoritesStore()
    private lazy var fluidBackground = fluidBackgroundStore.load()
    private lazy var menuBarSettings = menuBarSettingsStore.load()

    private let sidebarBackground = SidebarBackgroundView()
    private let sidebarStack = NSStackView()
    private var sidebarRows: [SidebarScope: SidebarTreeRowButton] = [:]
    private var appBackgroundWebView: WKWebView?
    private weak var contentSurface: NSView?
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
    private static let sidebarExpandedNodeIDsKey = "StarleeSidebarExpandedNodeIDs"
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
        favoriteIDs = favoritesStore.load()
        let savedExpandedIDs = UserDefaults.standard.stringArray(forKey: Self.sidebarExpandedNodeIDsKey)
        if let savedExpandedIDs, !savedExpandedIDs.isEmpty {
            expandedSidebarNodeIDs.formUnion(savedExpandedIDs)
        }
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
        let sidebar = makeSidebar()
        let main = makeMainPane()
        let content = NSView()
        content.translatesAutoresizingMaskIntoConstraints = false
        content.wantsLayer = true
        content.layer?.backgroundColor = NSColor.clear.cgColor
        contentSurface = content
        content.addSubview(sidebar)
        content.addSubview(main)

        let root = NSView()
        let background = makeAppBackgroundWebView()
        appBackgroundWebView = background
        root.addSubview(background)
        root.addSubview(content)
        NSLayoutConstraint.activate([
            background.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            background.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            background.topAnchor.constraint(equalTo: root.topAnchor),
            background.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            content.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            content.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            content.topAnchor.constraint(equalTo: root.topAnchor),
            content.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            sidebar.leadingAnchor.constraint(equalTo: content.leadingAnchor),
            sidebar.topAnchor.constraint(equalTo: content.topAnchor),
            sidebar.bottomAnchor.constraint(equalTo: content.bottomAnchor),
            sidebar.widthAnchor.constraint(equalToConstant: 300),
            main.leadingAnchor.constraint(equalTo: sidebar.trailingAnchor),
            main.trailingAnchor.constraint(equalTo: content.trailingAnchor),
            main.topAnchor.constraint(equalTo: content.topAnchor),
            main.bottomAnchor.constraint(equalTo: content.bottomAnchor)
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
        let sidebar = sidebarBackground
        sidebar.translatesAutoresizingMaskIntoConstraints = false
        sidebar.wantsLayer = true

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .width
        stack.spacing = 16
        stack.edgeInsets = NSEdgeInsets(top: 24, left: 18, bottom: 18, right: 18)
        stack.translatesAutoresizingMaskIntoConstraints = false
        sidebar.addSubview(stack)

        let wordmark = NSImageView()
        wordmark.image = Bundle.main.url(forResource: "StarleeWordmark", withExtension: "png")
            .flatMap(NSImage.init(contentsOf:))
        wordmark.imageScaling = .scaleProportionallyUpOrDown
        wordmark.translatesAutoresizingMaskIntoConstraints = false
        wordmark.heightAnchor.constraint(equalToConstant: 84).isActive = true
        stack.addArrangedSubview(wordmark)

        sidebarStack.orientation = .vertical
        sidebarStack.alignment = .leading
        sidebarStack.spacing = 3
        sidebarStack.edgeInsets = NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 4)
        sidebarStack.translatesAutoresizingMaskIntoConstraints = false
        stack.addArrangedSubview(sidebarStack)
        stack.addArrangedSubview(NSView())

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
            stack.topAnchor.constraint(equalTo: sidebar.topAnchor),
            stack.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor)
        ])
        rebuildSidebarTree()
        return sidebar
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

        titleLabel.font = .inter(ofSize: 34, weight: .heavy)
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

        readinessLabel.font = .inter(ofSize: 13)
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
        let chromeSetup = bridge["browser_setup"] as? [String: Any] ?? bridge["chrome_setup"] as? [String: Any] ?? [:]
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
            "menuBar": menuBarSettings.webPayload,
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
        title.font = .inter(ofSize: 24, weight: .heavy)
        title.textColor = Self.starleeWhite
        let subtitle = NSTextField(labelWithString: subtitleText(for: kind))
        subtitle.font = .inter(ofSize: 13, weight: .semibold)
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
        label.font = .inter(ofSize: 11, weight: .heavy)
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
                    .font: NSFont.inter(ofSize: 12, weight: .heavy)
                ]
            )
        } else {
            button.attributedTitle = NSAttributedString(
                string: look.name,
                attributes: [
                    .foregroundColor: Self.starleeBlack,
                    .font: NSFont.inter(ofSize: 12, weight: .bold)
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
        finishLabel.font = .inter(ofSize: 12, weight: .bold)
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
        label.font = .inter(ofSize: 12, weight: .semibold)
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
        modeLabel.font = .inter(ofSize: 12, weight: .bold)
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
        label.font = .inter(ofSize: 12, weight: .bold)
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
        label.font = .inter(ofSize: 12, weight: .bold)
        label.textColor = Self.starleeWhite
        label.widthAnchor.constraint(equalToConstant: 110).isActive = true

        let slider = NSSlider(value: value, minValue: min, maxValue: max, target: self, action: action)
        slider.widthAnchor.constraint(equalToConstant: 190).isActive = true

        let valueLabel = NSTextField(labelWithString: formattedFluidValue(value))
        valueLabel.font = .inter(ofSize: 12, weight: .bold)
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
        titleLabel.font = .inter(ofSize: 18, weight: .heavy)
        titleLabel.textColor = Self.starleeWhite
        let detailLabel = NSTextField(wrappingLabelWithString: detail.isEmpty ? "No detail available." : detail)
        detailLabel.font = .inter(ofSize: 12, weight: .semibold)
        detailLabel.textColor = Self.starleeCream
        text.addArrangedSubview(titleLabel)
        text.addArrangedSubview(detailLabel)

        let statusLabel = NSTextField(labelWithString: status)
        statusLabel.font = .inter(ofSize: 12, weight: .heavy)
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
            let captures = self.enrichTaxonomy(recent.map(Self.capture(from:)))
            DispatchQueue.main.async {
                self.doctor = doctor
                self.captures = captures
                self.groups = Self.monthGroups(from: captures)
                self.progress.stopAnimation(nil)
                self.isReloading = false
                self.rebuildSidebarTree()
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
            topics: (value["topics"] as? [String]) ?? [],
            taxonomyTopics: [],
            companies: [],
            countries: (value["countries"] as? [String]) ?? [],
            customEntities: (value["custom_entities"] as? [String]) ?? (value["customEntities"] as? [String]) ?? [],
            favorite: false
        )
    }

    private func enrichTaxonomy(_ captures: [LibraryCapture]) -> [LibraryCapture] {
        let payload = captures.map { capture in
            [
                "id": capture.id,
                "title": capture.title,
                "source": capture.source,
                "site": capture.site ?? "",
                "author": capture.author ?? "",
                "snippet": capture.snippet,
                "topics": capture.topics
            ] as [String: Any]
        }
        guard
            let scriptURL = Bundle.main.url(forResource: "extract_taxonomy", withExtension: "py", subdirectory: "taxonomy"),
            JSONSerialization.isValidJSONObject(payload),
            let input = try? JSONSerialization.data(withJSONObject: payload, options: [])
        else {
            return captures.map(enrichTaxonomyFallback)
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        process.arguments = [scriptURL.path]
        let stdin = Pipe()
        let stdout = Pipe()
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = Pipe()
        do {
            try process.run()
            stdin.fileHandleForWriting.write(input)
            try? stdin.fileHandleForWriting.close()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else {
                return captures.map(enrichTaxonomyFallback)
            }
            let data = stdout.fileHandleForReading.readDataToEndOfFile()
            guard
                let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                let items = value["items"] as? [[String: Any]]
            else {
                return captures.map(enrichTaxonomyFallback)
            }
            let byID = Dictionary(uniqueKeysWithValues: items.compactMap { item -> (String, ([String], [String]))? in
                guard let id = item["id"] as? String else { return nil }
                return (id, ((item["topics"] as? [String]) ?? [], (item["companies"] as? [String]) ?? []))
            })
            return captures.map { capture in
                let generated = byID[capture.id]
                let fallback = enrichTaxonomyFallback(capture)
                return LibraryCapture(
                    id: capture.id,
                    title: capture.title,
                    type: capture.type,
                    site: capture.site,
                    author: capture.author,
                    url: capture.url,
                    capturedAt: capture.capturedAt,
                    capturedAtText: capture.capturedAtText,
                    filePath: capture.filePath,
                    snippet: capture.snippet,
                    topics: capture.topics,
                    taxonomyTopics: generated?.0 ?? fallback.taxonomyTopics,
                    companies: generated?.1 ?? fallback.companies,
                    countries: capture.countries,
                    customEntities: capture.customEntities,
                    favorite: favoriteIDs.contains(capture.id)
                )
            }
        } catch {
            return captures.map(enrichTaxonomyFallback)
        }
    }

    private func enrichTaxonomyFallback(_ capture: LibraryCapture) -> LibraryCapture {
        let text = " " + ([capture.title, capture.source, capture.author ?? "", capture.site ?? "", capture.snippet] + capture.topics)
            .joined(separator: " ")
            .lowercased() + " "
        var topics: [String] = []
        func add(_ topic: String, when needles: [String]) {
            guard needles.contains(where: { text.contains($0) }), !topics.contains(topic) else { return }
            topics.append(topic)
        }
        add("Tech / AI", when: [" ai ", "artificial intelligence", "machine learning", "model", "llm", "openai", "anthropic", "claude", "chatgpt"])
        add("Tech / AI Infrastructure", when: ["ai infrastructure", "data center", "datacenter", "compute cluster", "gpu cluster", "accelerator"])
        add("Tech / Enterprise SaaS", when: ["figma", "salesforce", "enterprise software", "enterprise", "saas", "b2b software", "design tool"])
        add("Tech / Semiconductors", when: ["semiconductor", "chip", "chips", "gpu", "nvidia", "tsmc", "amd", "foundry", "wafer"])
        add("Tech / Fintech", when: ["fintech", "stripe", "payments", "banking", "neobank", "lending", "stablecoin"])
        add("Tech / Consumer Hardware", when: ["iphone", "ipad", "apple watch", "vision pro", "wearable", "consumer hardware", "device"])
        add("Tech / Robotics", when: ["robot", "robots", "robotics", "humanoid", "autonomous", "drone"])
        add("Tech / Digital Advertising", when: ["advertising", "adtech", "ads", "digital ads", "performance marketing", "targeting"])
        add("Tech / E-commerce", when: ["e-commerce", "ecommerce", "shopify", "marketplace", "online shopping", "merchant"])
        add("Tech / Cybersecurity", when: ["cybersecurity", "security", "ransomware", "malware", "phishing", "zero trust", "breach"])
        add("Politics / Presidency", when: ["president", "presidency", "white house", "executive order", "administration", "trump", "biden"])
        add("Politics / House", when: ["house of representatives", "speaker of the house", "congressman", "congresswoman", "house republican", "house democrat"])
        add("Politics / Senate", when: ["senate", "senator", "filibuster", "majority leader", "minority leader"])
        add("Politics / Elections", when: ["election", "campaign", "primary", "polling", "ballot", "voter", "electoral"])
        add("Politics / Middle East", when: ["middle east", "israel", "iran", "gaza", "hamas", "hezbollah", "netanyahu", "saudi arabia"])
        add("Business / Markets", when: ["stock market", "markets", "earnings", "revenue", "profit", "ipo", "valuation", "acquisition", "merger"])
        add("Business / Oil & Gas", when: [" oil ", " gas ", "opec", "crude", "lng", "shale", "refinery", "energy market"])
        add("Business / Retail", when: ["retail", "consumer spending", "store", "stores", "brand", "supply chain"])
        add("News / General", when: ["breaking", "report", "reported", "according to", "new york times", "washington post", "associated press", "reuters", "bloomberg"])
        let knownCompanies = ["AMD", "Adobe", "Airbnb", "Amazon", "Anthropic", "Apple", "Cursor", "Databricks", "Figma", "Google", "Intel", "Meta", "Microsoft", "Netflix", "NVIDIA", "OpenAI", "Palantir", "Salesforce", "Shopify", "SpaceX", "Stripe", "Tesla", "TSMC"]
        let companies = knownCompanies.filter { company in
            text.range(of: #"(?<![A-Za-z0-9])\#(NSRegularExpression.escapedPattern(for: company))(?![A-Za-z0-9])"#, options: [.regularExpression, .caseInsensitive]) != nil
        }
        return LibraryCapture(
            id: capture.id,
            title: capture.title,
            type: capture.type,
            site: capture.site,
            author: capture.author,
            url: capture.url,
            capturedAt: capture.capturedAt,
            capturedAtText: capture.capturedAtText,
            filePath: capture.filePath,
            snippet: capture.snippet,
            topics: capture.topics,
            taxonomyTopics: topics.isEmpty ? ["News / General"] : topics,
            companies: companies,
            countries: capture.countries,
            customEntities: capture.customEntities,
            favorite: favoriteIDs.contains(capture.id)
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

    private func rebuildSidebarTree() {
        sidebarStack.arrangedSubviews.forEach { view in
            sidebarStack.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        sidebarRows.removeAll()
        renderSidebarNodes(sidebarTree(), indent: 0)
        sidebarBackground.needsDisplay = true
    }

    private func renderSidebarNodes(_ nodes: [NavNode], indent: Int) {
        for node in nodes {
            let expanded = expandedSidebarNodeIDs.contains(node.id)
            let row = SidebarTreeRowButton(
                label: node.label,
                count: node.count,
                indent: indent,
                hasChildren: node.hasChildren,
                isExpanded: expanded,
                isSectionHeader: node.scope == nil && !node.hasChildren,
                isPrimaryLibrary: node.id == "my-library"
            )
            row.identifier = NSUserInterfaceItemIdentifier(node.id)
            row.scope = node.scope
            row.target = self
            row.action = #selector(selectSidebarNode(_:))
            row.setSelected(node.scope == selectedSidebarScope)
            sidebarStack.addArrangedSubview(row)
            if let scope = node.scope {
                sidebarRows[scope] = row
            }
            if node.hasChildren && expanded {
                renderSidebarNodes(node.children, indent: indent + 1)
            }
        }
    }

    private func sidebarTree() -> [NavNode] {
        var children: [NavNode] = []
        if menuBarSettings.favorites {
            children.append(NavNode(id: "favorites", label: "Favorites", count: captures.filter(\.favorite).count, scope: .favorites, children: []))
        }
        if menuBarSettings.topics, let node = optionalGroupNode(id: "topics", label: "Topics", children: topicNodes()) {
            children.append(node)
        }
        if menuBarSettings.time, let node = optionalGroupNode(id: "time", label: "Time", children: timeNodes()) {
            children.append(node)
        }
        if menuBarSettings.sources, let node = optionalGroupNode(id: "sources", label: "Sources", children: sourceNodes()) {
            children.append(node)
        }
        if menuBarSettings.companies, let node = optionalGroupNode(id: "companies", label: "Companies", children: companyNodes()) {
            children.append(node)
        }
        if menuBarSettings.countries, let node = optionalGroupNode(id: "countries", label: "Countries", children: countryNodes()) {
            children.append(node)
        }
        if menuBarSettings.customEntities, let node = optionalGroupNode(id: "custom-entities", label: "Custom Entities", children: customEntityNodes()) {
            children.append(node)
        }
        return [
            NavNode(id: "my-library", label: "My Library", count: captures.count, scope: .all, children: children)
        ]
    }

    private func optionalGroupNode(id: String, label: String, children: [NavNode]) -> NavNode? {
        if children.isEmpty && !menuBarSettings.showEmptySections {
            return nil
        }
        return NavNode(id: id, label: label, count: nil, scope: nil, children: children)
    }

    private func timeNodes() -> [NavNode] {
        groups.map { group in
            NavNode(id: "month:\(group.id)", label: group.label, count: group.captures.count, scope: .month(group.id), children: [])
        }
    }

    private func topicNodes() -> [NavNode] {
        let topics = captures.flatMap(\.taxonomyTopics)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        let counts = Dictionary(grouping: topics, by: { $0 }).mapValues(\.count)
        let roots = ["Tech", "Politics", "Business", "News"].filter { root in
            menuBarSettings.showEmptySections || topics.contains { $0 == root || $0.hasPrefix(root + " / ") }
        }
        return roots.map { root in
            let children = counts.keys
                .filter { $0.hasPrefix(root + " / ") }
                .filter { topic in
                    root == "Tech" || counts[topic, default: 0] >= 3
                }
                .sorted { lhs, rhs in
                    counts[lhs, default: 0] == counts[rhs, default: 0]
                        ? lhs.localizedCaseInsensitiveCompare(rhs) == .orderedAscending
                        : counts[lhs, default: 0] > counts[rhs, default: 0]
                }
                .prefix(10)
                .map { topic -> NavNode in
                    let label = topic.components(separatedBy: " / ").last ?? topic
                    return NavNode(id: "topic:\(topic)", label: label, count: counts[topic], scope: .topic(topic), children: [])
                }
            let rootCount = captures.filter { capture in
                capture.taxonomyTopics.contains { $0 == root || $0.hasPrefix(root + " / ") }
            }.count
            return NavNode(id: "topic:\(root)", label: root, count: rootCount, scope: .topic(root), children: children)
        }
    }

    private func sourceNodes() -> [NavNode] {
        let grouped = Dictionary(grouping: captures) { $0.sourceKind }
        return [
            sourceGroupNode(id: "source-articles", label: "Articles", captures: grouped["article"] ?? []),
            sourceGroupNode(id: "source-media", label: "Media", captures: grouped["media"] ?? [])
        ].compactMap { $0 }
    }

    private func companyNodes() -> [NavNode] {
        metadataNodes(
            idPrefix: "company",
            values: captures.flatMap(\.companies),
            scope: { .company($0) }
        )
    }

    private func countryNodes() -> [NavNode] {
        metadataNodes(
            idPrefix: "country",
            values: captures.flatMap(\.countries),
            scope: { .country($0) }
        )
    }

    private func customEntityNodes() -> [NavNode] {
        metadataNodes(
            idPrefix: "custom-entity",
            values: captures.flatMap(\.customEntities),
            scope: { .customEntity($0) }
        )
    }

    private func metadataNodes(
        idPrefix: String,
        values: [String],
        scope: (String) -> SidebarScope
    ) -> [NavNode] {
        let cleaned = values
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        let counts = Dictionary(grouping: cleaned, by: { $0 }).mapValues(\.count)
        return counts.keys
            .sorted { lhs, rhs in
                counts[lhs, default: 0] == counts[rhs, default: 0]
                    ? lhs.localizedCaseInsensitiveCompare(rhs) == .orderedAscending
                    : counts[lhs, default: 0] > counts[rhs, default: 0]
            }
            .prefix(12)
            .map { value in
                NavNode(id: "\(idPrefix):\(value)", label: value, count: counts[value], scope: scope(value), children: [])
            }
    }

    private func sourceGroupNode(id: String, label: String, captures: [LibraryCapture]) -> NavNode? {
        guard !captures.isEmpty else { return nil }
        let byKey = Dictionary(grouping: captures, by: \.sourceKey)
        let children = byKey.map { key, captures in
            let label = captures.first?.sourceLabel ?? "Unknown"
            return NavNode(id: "source:\(key)", label: label, count: captures.count, scope: .source(key), children: [])
        }
        .sorted { lhs, rhs in
            if (lhs.count ?? 0) == (rhs.count ?? 0) {
                return lhs.label.localizedCaseInsensitiveCompare(rhs.label) == .orderedAscending
            }
            return (lhs.count ?? 0) > (rhs.count ?? 0)
        }
        return NavNode(id: id, label: label, count: captures.count, scope: nil, children: children)
    }

    private func updateSidebarSelection() {
        for (scope, row) in sidebarRows {
            row.setSelected(scope == selectedSidebarScope)
        }
    }

    private func applyFilters() {
        let scopedCaptures = capturesForSelectedScope()
        let query = searchField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if query.isEmpty {
            filteredCaptures = scopedCaptures
        } else {
            filteredCaptures = scopedCaptures.filter { capture in
                ([capture.title, capture.sourceLabel, capture.source, capture.type, capture.snippet] + capture.taxonomyTopics)
                    .joined(separator: " ")
                    .lowercased()
                    .contains(query)
            }
        }
        tableView.reloadData()
    }

    private func capturesForSelectedScope() -> [LibraryCapture] {
        switch selectedSidebarScope {
        case .all:
            return captures
        case .favorites:
            return captures.filter(\.favorite)
        case .month(let id):
            return groups.first { $0.id == id }?.captures ?? captures
        case .topic(let topic):
            return captures.filter { capture in
                capture.taxonomyTopics.contains { $0 == topic || $0.hasPrefix(topic + " / ") }
            }
        case .source(let key):
            return captures.filter { $0.sourceKey == key }
        case .company(let company):
            return captures.filter { $0.companies.contains(company) }
        case .country(let country):
            return captures.filter { $0.countries.contains(country) }
        case .customEntity(let entity):
            return captures.filter { $0.customEntities.contains(entity) }
        case .settings:
            return captures
        }
    }

    private func libraryPayloadJSON() -> String {
        let scopedCaptures = capturesForSelectedScope()
        let ready = doctor?["ok"] as? Bool == true
        let readiness = ready ? "Ready" : "Needs setup"
        let monthLabel = selectedScopeLabel()
        let payload: [String: Any] = [
            "monthLabel": monthLabel,
            "totalCount": captures.count,
            "readiness": readiness,
            "showOnboarding": !UserDefaults.standard.bool(forKey: Self.onboardingCompleteKey),
            "backgroundSettings": fluidBackground.webPayload,
            "captures": scopedCaptures.map { capture in
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
                    "topics": capture.taxonomyTopics,
                    "companies": capture.companies,
                    "countries": capture.countries,
                    "customEntities": capture.customEntities,
                    "favorite": capture.favorite
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

    private func selectedScopeLabel() -> String {
        switch selectedSidebarScope {
        case .all:
            return "My Library"
        case .favorites:
            return "Favorites"
        case .month(let id):
            return groups.first { $0.id == id }?.label ?? "My Library"
        case .topic(let topic):
            return topic.components(separatedBy: " / ").last ?? topic
        case .source(let key):
            return captures.first { $0.sourceKey == key }?.sourceLabel ?? "Source"
        case .company(let company):
            return company
        case .country(let country):
            return country
        case .customEntity(let entity):
            return entity
        case .settings:
            return "Settings"
        }
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
        field.font = .inter(ofSize: identifier == "title" ? 13 : 12)
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
        case "setMenuBarSettings":
            if let settings = body["settings"] as? [String: Any] {
                menuBarSettings = MenuBarSettings(payload: settings)
                saveAndApplyMenuBarSettings()
            }
        case "resetMenuBarSettings":
            menuBarSettings = .default
            saveAndApplyMenuBarSettings()
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
        case "toggleFavorite":
            if let id = body["id"] as? String {
                setFavorite(id: id, favorite: (body["favorite"] as? Bool) ?? false)
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
        case "settings":
            showSettings()
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

    private func setFavorite(id: String, favorite: Bool) {
        if favorite {
            favoriteIDs.insert(id)
        } else {
            favoriteIDs.remove(id)
        }
        favoritesStore.save(favoriteIDs)
        captures = captures.map { capture in
            guard capture.id == id else { return capture }
            return LibraryCapture(
                id: capture.id,
                title: capture.title,
                type: capture.type,
                site: capture.site,
                author: capture.author,
                url: capture.url,
                capturedAt: capture.capturedAt,
                capturedAtText: capture.capturedAtText,
                filePath: capture.filePath,
                snippet: capture.snippet,
                topics: capture.topics,
                taxonomyTopics: capture.taxonomyTopics,
                companies: capture.companies,
                countries: capture.countries,
                customEntities: capture.customEntities,
                favorite: favorite
            )
        }
        if selectedSidebarScope == .favorites && favorite == false {
            applyFilters()
        }
        rebuildSidebarTree()
        renderLibraryPayload()
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

    /// Bulk-import user documents (PDF, Word, text, Markdown), then reload the
    /// Library. Parsing happens locally in the CLI; topic organization is
    /// generated by the Library taxonomy layer.
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
            var arguments = ["import"]
            arguments.append(contentsOf: urls.map { $0.path })
            self.progress.startAnimation(nil)
            self.client.runAsync(arguments) { [weak self] output in
                guard let self else { return }
                self.reportImport(output: output, total: urls.count)
                self.reload()
            }
        }
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
        if let capture, !capture.taxonomyTopics.isEmpty {
            reader["topics"] = capture.taxonomyTopics
            reader["companies"] = capture.companies
        } else if let topics = metadata["topics"] as? [String], !topics.isEmpty {
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
                self.favoriteIDs.remove(id)
                self.favoritesStore.save(self.favoriteIDs)
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
            let browser = bridge["browser"] as? String ?? "Browser"
            lines.append("\(browser) extension \(version) (\(build))")
        }
        if let passedAt = setup["capture_test_passed_at"] as? String {
            lines.append("Capture test: \(passedAt)")
        }
        if lines.isEmpty {
            return "Chrome uses the local extension folder. Firefox and Safari are disabled while capture is being stabilized."
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
        button.font = .inter(ofSize: 12, weight: .bold)
        button.heightAnchor.constraint(equalToConstant: 26).isActive = true
        button.widthAnchor.constraint(greaterThanOrEqualToConstant: 74).isActive = true
        button.attributedTitle = NSAttributedString(
            string: button.title,
            attributes: [
                .foregroundColor: Self.starleeBlack,
                .font: NSFont.inter(ofSize: 12, weight: .bold)
            ]
        )
    }

    private func applyFluidBackground() {
        window?.appearance = NSAppearance(named: .aqua)
        contentSurface?.layer?.backgroundColor = NSColor.clear.cgColor
        tableView.backgroundColor = NSColor.controlBackgroundColor.withAlphaComponent(0.72)
        tableView.enclosingScrollView?.backgroundColor = tableView.backgroundColor
        updateFluidBackgroundControls()
        updateFluidBackgroundRenderers()
    }

    private func saveAndApplyFluidBackground() {
        fluidBackgroundStore.save(fluidBackground)
        applyFluidBackground()
    }

    private func saveAndApplyMenuBarSettings() {
        menuBarSettingsStore.save(menuBarSettings)
        if !isSidebarScopeEnabled(selectedSidebarScope) {
            selectedSidebarScope = .all
            primaryView = .library
        }
        rebuildSidebarTree()
        renderLibraryPayload()
        renderSettingsPayload()
    }

    private func isSidebarScopeEnabled(_ scope: SidebarScope) -> Bool {
        switch scope {
        case .all, .settings:
            return true
        case .favorites:
            return menuBarSettings.favorites
        case .topic:
            return menuBarSettings.topics
        case .month:
            return menuBarSettings.time
        case .source:
            return menuBarSettings.sources
        case .company:
            return menuBarSettings.companies
        case .country:
            return menuBarSettings.countries
        case .customEntity:
            return menuBarSettings.customEntities
        }
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
        selectedSidebarScope = .all
        render()
    }

    @objc private func showSettings() {
        primaryView = .settings
        selectedSidebarScope = .settings
        render()
    }

    @objc private func selectSidebarNode(_ sender: SidebarTreeRowButton) {
        if sender.hasChildren {
            let id = sender.identifier?.rawValue ?? ""
            if expandedSidebarNodeIDs.contains(id) {
                expandedSidebarNodeIDs.remove(id)
            } else {
                expandedSidebarNodeIDs.insert(id)
            }
            UserDefaults.standard.set(Array(expandedSidebarNodeIDs).sorted(), forKey: Self.sidebarExpandedNodeIDsKey)
            rebuildSidebarTree()
        }
        guard let scope = sender.scope else { return }
        selectedSidebarScope = scope
        primaryView = scope == .settings ? .settings : .library
        rebuildSidebarTree()
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

private final class SidebarTreeRowButton: NSButton {
    private static let navy = NSColor(srgbRed: 19.0 / 255.0, green: 40.0 / 255.0, blue: 75.0 / 255.0, alpha: 1)
    private static let cream = NSColor(srgbRed: 242.0 / 255.0, green: 227.0 / 255.0, blue: 182.0 / 255.0, alpha: 1)
    private static let white = NSColor.white
    private static let black = NSColor.black

    let rowLabel: String
    let count: Int?
    let indent: Int
    let hasChildren: Bool
    let isSectionHeader: Bool
    let isPrimaryLibrary: Bool
    var scope: SidebarScope?
    private var isExpanded: Bool
    private var isHovering = false
    private var isPressing = false
    private var isSelectedRow = false
    private var trackingAreaRef: NSTrackingArea?

    init(
        label: String,
        count: Int?,
        indent: Int,
        hasChildren: Bool,
        isExpanded: Bool,
        isSectionHeader: Bool,
        isPrimaryLibrary: Bool
    ) {
        self.rowLabel = label
        self.count = count
        self.indent = indent
        self.hasChildren = hasChildren
        self.isExpanded = isExpanded
        self.isSectionHeader = isSectionHeader
        self.isPrimaryLibrary = isPrimaryLibrary
        super.init(frame: .zero)
        isBordered = false
        bezelStyle = .regularSquare
        focusRingType = .none
        setButtonType(.momentaryChange)
        translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 260).isActive = true
        heightAnchor.constraint(equalToConstant: isSectionHeader ? 26 : 40).isActive = true
    }

    required init?(coder: NSCoder) {
        nil
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
        isPressing = true
        needsDisplay = true
        super.mouseDown(with: event)
        isPressing = false
        needsDisplay = true
    }

    func setSelected(_ selected: Bool) {
        guard isSelectedRow != selected else { return }
        isSelectedRow = selected
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        if isSectionHeader {
            drawSectionHeader()
            return
        }

        var rect = bounds.insetBy(dx: isPrimaryLibrary ? 1.5 : 3, dy: 3)
        if isPressing {
            rect = rect.offsetBy(dx: 1, dy: -1)
        }
        if isPrimaryLibrary || isSelectedRow || isHovering {
            let radius = min(12, rect.height / 2.6)
            let surface = NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius)
            if isSelectedRow {
                NSGraphicsContext.saveGraphicsState()
                let shadow = NSShadow()
                shadow.shadowColor = Self.black.withAlphaComponent(0.18)
                shadow.shadowOffset = NSSize(width: 0, height: -1)
                shadow.shadowBlurRadius = 4
                shadow.set()
                Self.cream.setFill()
                surface.fill()
                NSGraphicsContext.restoreGraphicsState()
            }
            if isSelectedRow {
                Self.cream.setFill()
            } else if isPrimaryLibrary {
                Self.white.withAlphaComponent(isPressing ? 0.18 : 0.14).setFill()
            } else {
                Self.white.withAlphaComponent(isPressing ? 0.13 : 0.09).setFill()
            }
            surface.fill()
            if isSelectedRow || isPrimaryLibrary {
                (isSelectedRow ? Self.black.withAlphaComponent(0.18) : Self.cream.withAlphaComponent(0.72)).setStroke()
                surface.lineWidth = isSelectedRow ? 1.1 : 1
                surface.stroke()
            }
        }

        drawDisclosure()
        drawLabel()
    }

    private func drawSectionHeader() {
        let text = rowLabel.uppercased()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.inter(ofSize: 10, weight: .heavy),
            .foregroundColor: Self.cream.withAlphaComponent(0.76),
            .kern: 1.2
        ]
        let attributed = NSAttributedString(string: text, attributes: attributes)
        attributed.draw(at: NSPoint(x: 10, y: bounds.midY - attributed.size().height / 2))
    }

    private func drawDisclosure() {
        guard hasChildren else { return }
        let symbol = isExpanded ? "▾" : "▸"
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 11, weight: .bold),
            .foregroundColor: isSelectedRow ? Self.black.withAlphaComponent(0.76) : Self.cream.withAlphaComponent(0.86)
        ]
        let attributed = NSAttributedString(string: symbol, attributes: attributes)
        attributed.draw(at: NSPoint(x: CGFloat(10 + indent * 15), y: bounds.midY - attributed.size().height / 2 + 0.5))
    }

    private func drawLabel() {
        let x = CGFloat(12 + indent * 15 + (hasChildren ? 17 : 0))
        let trailingPadding: CGFloat = 12
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail
        let fontSize: CGFloat = isPrimaryLibrary ? 14.5 : 12.5
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.inter(ofSize: fontSize, weight: isPrimaryLibrary ? .heavy : .semibold),
            .foregroundColor: isSelectedRow ? Self.black : Self.white.withAlphaComponent(isPrimaryLibrary ? 1 : 0.94),
            .kern: 0,
            .paragraphStyle: paragraph
        ]
        let text = NSAttributedString(string: rowLabel, attributes: attributes)
        let maxWidth = max(20, bounds.width - x - trailingPadding)
        text.draw(in: NSRect(x: x, y: bounds.midY - text.size().height / 2, width: maxWidth, height: text.size().height))
    }
}

private final class SidebarBackgroundView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        NSColor.black.setFill()
        bounds.fill()
    }
}
