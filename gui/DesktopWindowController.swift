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
        let url: URL?
        let capturedAt: Date?
        let capturedAtText: String
        let filePath: String
        let snippet: String

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

    private let libraryButton = SidebarBoxButton(title: "Library")
    private let settingsButton = SidebarBoxButton(title: "Settings")
    private let monthStack = NSStackView()
    private var monthButtons: [String: NSButton] = [:]
    private var appBackgroundWebView: WKWebView?
    private weak var rootSplitView: NSSplitView?
    private weak var pixelColorWell: NSColorWell?
    private weak var backgroundColorWell: NSColorWell?
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
    private let tableView = NSTableView()
    private var libraryWebView: WKWebView?
    private var libraryWebViewLoaded = false
    private var pendingLibraryPayload: String?
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
            sidebar.widthAnchor.constraint(equalToConstant: 240)
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
        let sidebar = NSView()
        sidebar.translatesAutoresizingMaskIntoConstraints = false
        sidebar.wantsLayer = true
        sidebar.layer?.backgroundColor = NSColor.black.cgColor

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .width
        stack.spacing = 22
        stack.edgeInsets = NSEdgeInsets(top: 24, left: 16, bottom: 16, right: 16)
        stack.translatesAutoresizingMaskIntoConstraints = false
        sidebar.addSubview(stack)

        let wordmark = NSImageView()
        wordmark.image = Bundle.main.url(forResource: "StarleeWordmark", withExtension: "png")
            .flatMap(NSImage.init(contentsOf:))
        wordmark.imageScaling = .scaleProportionallyUpOrDown
        wordmark.translatesAutoresizingMaskIntoConstraints = false
        wordmark.heightAnchor.constraint(equalToConstant: 85).isActive = true
        stack.addArrangedSubview(wordmark)

        configureSidebarButton(libraryButton, action: #selector(showLibrary))
        configureSidebarButton(settingsButton, action: #selector(showSettings))

        let navStack = NSStackView(views: [libraryButton, settingsButton])
        navStack.orientation = .vertical
        navStack.alignment = .width
        navStack.spacing = 18
        stack.addArrangedSubview(navStack)

        let divider = NSView()
        divider.wantsLayer = true
        divider.layer?.backgroundColor = NSColor(calibratedRed: 0.949, green: 0.890, blue: 0.714, alpha: 0.86).cgColor
        divider.translatesAutoresizingMaskIntoConstraints = false
        divider.heightAnchor.constraint(equalToConstant: 1).isActive = true
        stack.addArrangedSubview(divider)

        monthStack.orientation = .vertical
        monthStack.alignment = .width
        monthStack.spacing = 18
        stack.addArrangedSubview(monthStack)
        stack.addArrangedSubview(NSView())

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
            stack.topAnchor.constraint(equalTo: sidebar.topAnchor),
            stack.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor)
        ])
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
        headerView?.isHidden = primaryView == .library
        contentStack.spacing = primaryView == .library ? 0 : 14
        contentStack.edgeInsets = primaryView == .library
            ? NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
            : NSEdgeInsets(top: 22, left: 24, bottom: 20, right: 24)
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
        titleLabel.stringValue = "Settings"
        titleLabel.textColor = Self.starleeBlack
        subtitleLabel.stringValue = "Setup, diagnostics, vault, import/export, and repair."
        readinessLabel.textColor = Self.starleeWhite
        let checks = checksByName()
        let bridge = (status()["bridge_health"] as? [String: Any]) ?? [:]
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"

        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 500).isActive = true

        let settingsStack = NSStackView()
        settingsStack.orientation = .vertical
        settingsStack.alignment = .width
        settingsStack.spacing = 18
        settingsStack.edgeInsets = NSEdgeInsets(top: 2, left: 2, bottom: 24, right: 10)
        settingsStack.translatesAutoresizingMaskIntoConstraints = false
        scroll.documentView = settingsStack

        settingsStack.addArrangedSubview(appearancePanel())
        settingsStack.addArrangedSubview(settingsCard(
            title: "Browser Extensions",
            status: (bridge["ok"] as? Bool) == true ? "Ready" : "Needs attention",
            detail: bridge["recommended_next_action"] as? String ?? "Chrome uses the local extension folder. Safari uses the Starlee Capture extension wrapper.",
            actionTitle: "Open Setup",
            action: #selector(openBrowserSetup)
        ))
        settingsStack.addArrangedSubview(settingsCard(
            title: "Codex Plugin",
            status: checks["codex_plugin_source"]?.ok == true ? "Installed" : "Needs setup",
            detail: checks["codex_plugin_source"]?.detail ?? "Install or repair the local Starlee Codex plugin.",
            actionTitle: "Guide",
            action: #selector(showCodexGuide)
        ))
        settingsStack.addArrangedSubview(settingsCard(
            title: "Diagnostics",
            status: doctor?["ok"] as? Bool == true ? "Ready" : "Needs attention",
            detail: ((doctor?["next_actions"] as? [String]) ?? []).first ?? "Endpoint, bridge, and vault checks are healthy.",
            actionTitle: "Copy Redacted",
            action: #selector(copySupportBundle)
        ))
        settingsStack.addArrangedSubview(settingsCard(
            title: "Vault",
            status: checks["vault"]?.ok == true ? "Local" : "Missing",
            detail: statusString("vault"),
            actionTitle: "Open",
            action: #selector(openVault)
        ))
        settingsStack.addArrangedSubview(settingsCard(
            title: "Import / Export",
            status: "Local",
            detail: "Import plain text or Markdown into the vault. Export remains available through the audited CLI bundle flow.",
            actionTitle: "Import",
            action: #selector(importDocument)
        ))
        settingsStack.addArrangedSubview(settingsCard(
            title: "App Version",
            status: version,
            detail: "Starlee desktop app and menu-bar capture surface.",
            actionTitle: nil,
            action: nil
        ))

        NSLayoutConstraint.activate([
            settingsStack.leadingAnchor.constraint(equalTo: scroll.contentView.leadingAnchor),
            settingsStack.trailingAnchor.constraint(equalTo: scroll.contentView.trailingAnchor),
            settingsStack.topAnchor.constraint(equalTo: scroll.contentView.topAnchor),
            settingsStack.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor)
        ])
        contentStack.addArrangedSubview(scroll)
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

        let titleStack = NSStackView()
        titleStack.orientation = .vertical
        titleStack.spacing = 4
        let title = NSTextField(labelWithString: "Background")
        title.font = .systemFont(ofSize: 24, weight: .heavy)
        title.textColor = Self.starleeWhite
        let subtitle = NSTextField(labelWithString: "Fluid pixel-dither background · saved instantly")
        subtitle.font = .systemFont(ofSize: 13, weight: .semibold)
        subtitle.textColor = Self.starleeCream
        titleStack.addArrangedSubview(title)
        titleStack.addArrangedSubview(subtitle)
        stack.addArrangedSubview(titleStack)

        let colorRow = NSStackView()
        colorRow.orientation = .horizontal
        colorRow.alignment = .centerY
        colorRow.spacing = 18
        let pixelColor = colorControl(title: "Pixel color", hex: fluidBackground.pixelColor, action: #selector(changePixelColor(_:)))
        pixelColorWell = pixelColor.well
        let backgroundColor = colorControl(title: "Background color", hex: fluidBackground.backgroundColor, action: #selector(changeBackgroundColor(_:)))
        backgroundColorWell = backgroundColor.well
        colorRow.addArrangedSubview(pixelColor.view)
        colorRow.addArrangedSubview(backgroundColor.view)
        stack.addArrangedSubview(colorRow)

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
        stack.addArrangedSubview(controls)

        let looks = NSStackView()
        looks.orientation = .horizontal
        looks.alignment = .centerY
        looks.spacing = 8
        for look in FluidBackgroundLooks.all {
            let button = NSButton(title: look.name, target: self, action: #selector(selectFluidLook(_:)))
            styleSettingsActionButton(button)
            button.attributedTitle = NSAttributedString(
                string: look.name,
                attributes: [
                    .foregroundColor: Self.starleeBlack,
                    .font: NSFont.systemFont(ofSize: 12, weight: .bold)
                ]
            )
            button.identifier = NSUserInterfaceItemIdentifier(look.name)
            looks.addArrangedSubview(button)
        }
        stack.addArrangedSubview(looks)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: box.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: box.trailingAnchor),
            stack.topAnchor.constraint(equalTo: box.topAnchor),
            stack.bottomAnchor.constraint(equalTo: box.bottomAnchor)
        ])

        updateFluidBackgroundControls()
        return box
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
            url: urlString.flatMap(URL.init(string:)),
            capturedAt: parseDate(dateText),
            capturedAtText: dateText,
            filePath: value["file_path"] as? String ?? "",
            snippet: value["snippet"] as? String ?? ""
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
        if groups.isEmpty {
            let empty = NSTextField(labelWithString: "No captures yet")
            empty.font = SidebarBoxButton.labelFont
            empty.textColor = .white
            monthStack.addArrangedSubview(empty)
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
            "backgroundSettings": fluidBackground.webPayload,
            "captures": monthCaptures.map { capture in
                [
                    "id": capture.id,
                    "title": capture.title,
                    "type": capture.type,
                    "source": capture.source,
                    "date": displayDate(capture.capturedAt, fallback: capture.capturedAtText),
                    "snippet": capture.snippet,
                    "url": capture.url?.absoluteString ?? "",
                    "filePath": capture.filePath
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
        libraryWebViewLoaded = true
        let payload = pendingLibraryPayload ?? libraryPayloadJSON()
        pendingLibraryPayload = nil
        webView.evaluateJavaScript("window.__starleeLibraryPayload = \(payload); if (window.renderStarleeLibrary) { window.renderStarleeLibrary(window.__starleeLibraryPayload); }", completionHandler: nil)
        updateFluidBackgroundRenderers()
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard
            message.name == "starlee",
            let body = message.body as? [String: Any],
            body["action"] as? String == "refresh"
        else { return }
        refresh()
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
        SidebarBoxButton.backgroundSettings = fluidBackground
        [libraryButton, settingsButton].forEach { $0.needsDisplay = true }
        monthButtons.values.forEach { $0.needsDisplay = true }
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
    }

    private func updateFluidBackgroundControls() {
        pixelColorWell?.color = FluidBackgroundSettings.color(from: fluidBackground.pixelColor)
        backgroundColorWell?.color = FluidBackgroundSettings.color(from: fluidBackground.backgroundColor)
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

private final class SidebarBoxButton: NSButton {
    static var backgroundSettings: FluidBackgroundSettings = .default

    static var labelFont: NSFont {
        NSFont(name: "Avenir Next Condensed Heavy", size: 21)
            ?? NSFont(name: "Avenir Next Heavy", size: 20)
            ?? NSFont(name: "Helvetica Neue Condensed Black", size: 20)
            ?? .systemFont(ofSize: 20, weight: .heavy)
    }

    private static let cardNavy = NSColor(calibratedRed: 19 / 255, green: 40 / 255, blue: 75 / 255, alpha: 1)
    private static let cream = NSColor(calibratedRed: 0.949, green: 0.890, blue: 0.714, alpha: 1)
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
        widthAnchor.constraint(equalToConstant: 208).isActive = true
        heightAnchor.constraint(equalToConstant: 76).isActive = true
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
        let pressOffset: CGFloat = isPressed ? -1 : 0
        let buttonRect = bounds.insetBy(dx: 2, dy: 4).offsetBy(dx: 0, dy: pressOffset)
        let buttonPath = NSBezierPath(roundedRect: buttonRect, xRadius: 8, yRadius: 8)
        let selected = isSelected || state == .on
        let surfaceAlpha: CGFloat = isPressed ? 0.88 : (selected ? 0.94 : (isHovering ? 0.92 : 0.85))
        let borderAlpha: CGFloat = selected ? 0.98 : (isHovering ? 0.86 : 0.42)

        NSGraphicsContext.saveGraphicsState()
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(isPressed ? 0.22 : 0.32)
        shadow.shadowBlurRadius = isPressed ? 8 : 16
        shadow.shadowOffset = NSSize(width: 0, height: isPressed ? -2 : -8)
        shadow.set()
        NSColor.black.withAlphaComponent(0.30).setFill()
        buttonPath.fill()
        NSGraphicsContext.restoreGraphicsState()

        NSGraphicsContext.saveGraphicsState()
        buttonPath.addClip()
        drawActiveBackgroundInfluence(in: buttonRect)
        Self.cardNavy.withAlphaComponent(surfaceAlpha).setFill()
        buttonPath.fill()

        let glossRect = NSRect(
            x: buttonRect.minX,
            y: buttonRect.midY + 1,
            width: buttonRect.width,
            height: buttonRect.height * 0.46
        )
        let glossPath = NSBezierPath()
        glossPath.move(to: NSPoint(x: buttonRect.minX, y: glossRect.maxY))
        glossPath.line(to: NSPoint(x: buttonRect.maxX, y: glossRect.maxY))
        glossPath.line(to: NSPoint(x: buttonRect.maxX, y: glossRect.minY + 8))
        glossPath.curve(
            to: NSPoint(x: buttonRect.minX, y: glossRect.minY + 18),
            controlPoint1: NSPoint(x: buttonRect.maxX * 0.72 + buttonRect.minX * 0.28, y: glossRect.minY - 10),
            controlPoint2: NSPoint(x: buttonRect.maxX * 0.25 + buttonRect.minX * 0.75, y: glossRect.minY + 1)
        )
        glossPath.close()
        glossPath.addClip()
        NSGradient(colors: [
            NSColor.white.withAlphaComponent(isPressed ? 0.018 : (isHovering ? 0.055 : 0.030)),
            NSColor.white.withAlphaComponent(0.006)
        ])?.draw(in: glossRect, angle: -90)
        NSGraphicsContext.restoreGraphicsState()

        NSColor.white.withAlphaComponent(borderAlpha).setStroke()
        buttonPath.lineWidth = selected ? 2.6 : 2
        buttonPath.stroke()

        Self.cream.withAlphaComponent(isHovering || selected ? 0.78 : 0.58).setStroke()
        let innerPath = NSBezierPath(roundedRect: buttonRect.insetBy(dx: 4, dy: 4), xRadius: 5, yRadius: 5)
        innerPath.lineWidth = 1
        innerPath.stroke()

        NSColor.black.withAlphaComponent(0.42).setStroke()
        let lowerInset = NSBezierPath()
        lowerInset.move(to: NSPoint(x: buttonRect.minX + 9, y: buttonRect.minY + 6))
        lowerInset.line(to: NSPoint(x: buttonRect.maxX - 9, y: buttonRect.minY + 6))
        lowerInset.lineWidth = 1
        lowerInset.stroke()

        if selected || isHovering || window?.firstResponder === self {
            NSColor.white.withAlphaComponent(selected ? 0.34 : (isHovering ? 0.26 : 0.44)).setStroke()
            let focusPath = NSBezierPath(roundedRect: buttonRect.insetBy(dx: -2, dy: -2), xRadius: 10, yRadius: 10)
            focusPath.lineWidth = isHovering ? 2 : 3
            focusPath.stroke()
        }

        drawIconAndTitle(in: buttonRect)
    }

    private func drawActiveBackgroundInfluence(in rect: NSRect) {
        let settings = Self.backgroundSettings
        let paper = FluidBackgroundSettings.color(from: settings.backgroundColor)
        let pixel = FluidBackgroundSettings.color(from: settings.pixelColor)
        paper.withAlphaComponent(0.34).setFill()
        NSBezierPath(rect: rect).fill()

        let block = max(3, CGFloat(settings.pixelSize))
        let columns = Int(rect.width / block) + 1
        let rows = Int(rect.height / block) + 1
        let threshold = UInt32(max(68, min(94, Int(settings.threshold * 160 + 35))))
        let seed: UInt32 = title == "Library" ? 137 : (title == "Settings" ? 251 : 389)
        for row in 0..<rows {
            for column in 0..<columns {
                let value = (UInt32(row * 37 + column * 53) &* seed) % 100
                guard value > threshold else { continue }
                pixel.withAlphaComponent(value > 94 ? 0.32 : 0.20).setFill()
                NSBezierPath(rect: NSRect(
                    x: rect.minX + CGFloat(column) * block,
                    y: rect.minY + CGFloat(row) * block,
                    width: min(block, rect.maxX - (rect.minX + CGFloat(column) * block)),
                    height: min(block, rect.maxY - (rect.minY + CGFloat(row) * block))
                )).fill()
            }
        }
    }

    private func drawIconAndTitle(in rect: NSRect) {
        let text = title.uppercased()
        let iconRect = NSRect(x: rect.minX + 28, y: rect.midY - 12, width: 24, height: 24)
        drawIcon(in: iconRect)

        let attributes: [NSAttributedString.Key: Any] = [
            .font: Self.labelFont,
            .foregroundColor: NSColor.white,
            .kern: 1.8,
            .shadow: textShadow
        ]
        let attributed = NSAttributedString(string: text, attributes: attributes)
        let textSize = attributed.size()
        let textRect = NSRect(
            x: rect.minX + 74,
            y: rect.midY - textSize.height / 2 + 1,
            width: max(0, rect.maxX - rect.minX - 86),
            height: textSize.height
        )
        attributed.draw(in: textRect)
    }

    private func drawIcon(in rect: NSRect) {
        NSGraphicsContext.saveGraphicsState()
        NSColor.white.setFill()
        NSColor.white.setStroke()

        switch title {
        case "Library":
            Self.cream.setFill()
            let book = NSBezierPath(roundedRect: rect.insetBy(dx: 3, dy: 1), xRadius: 2, yRadius: 2)
            book.fill()
            NSColor.black.withAlphaComponent(0.55).setFill()
            NSBezierPath(rect: NSRect(x: rect.minX + 8, y: rect.minY + 5, width: 2, height: rect.height - 10)).fill()
            NSColor.black.withAlphaComponent(0.35).setStroke()
            let pageLine = NSBezierPath()
            pageLine.move(to: NSPoint(x: rect.minX + 10, y: rect.maxY - 6))
            pageLine.line(to: NSPoint(x: rect.maxX - 5, y: rect.maxY - 6))
            pageLine.lineWidth = 1.2
            pageLine.stroke()
        case "Settings":
            let center = NSPoint(x: rect.midX, y: rect.midY)
            for index in 0..<8 {
                let angle = CGFloat(index) * .pi / 4
                let toothCenter = NSPoint(
                    x: center.x + cos(angle) * 8,
                    y: center.y + sin(angle) * 8
                )
                NSBezierPath(ovalIn: NSRect(x: toothCenter.x - 2.6, y: toothCenter.y - 2.6, width: 5.2, height: 5.2)).fill()
            }
            NSBezierPath(ovalIn: rect.insetBy(dx: 5, dy: 5)).fill()
            NSColor.black.withAlphaComponent(0.58).setFill()
            NSBezierPath(ovalIn: rect.insetBy(dx: 10, dy: 10)).fill()
        default:
            let calendar = NSBezierPath(roundedRect: rect.insetBy(dx: 2, dy: 3), xRadius: 2, yRadius: 2)
            calendar.lineWidth = 2
            calendar.stroke()
            NSBezierPath(rect: NSRect(x: rect.minX + 4, y: rect.maxY - 8, width: rect.width - 8, height: 2)).fill()
            NSBezierPath(roundedRect: NSRect(x: rect.minX + 7, y: rect.maxY - 4, width: 3, height: 6), xRadius: 1, yRadius: 1).fill()
            NSBezierPath(roundedRect: NSRect(x: rect.maxX - 10, y: rect.maxY - 4, width: 3, height: 6), xRadius: 1, yRadius: 1).fill()
            NSBezierPath(rect: NSRect(x: rect.minX + 8, y: rect.minY + 8, width: 3, height: 3)).fill()
            NSBezierPath(rect: NSRect(x: rect.minX + 14, y: rect.minY + 8, width: 3, height: 3)).fill()
        }
        NSGraphicsContext.restoreGraphicsState()
    }

    private var textShadow: NSShadow {
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.58)
        shadow.shadowBlurRadius = 2.5
        shadow.shadowOffset = NSSize(width: 0, height: -1)
        return shadow
    }
}
