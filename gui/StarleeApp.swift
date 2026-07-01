import AppKit
import UserNotifications

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let client = StarleeClient()
    private let notifier = NotificationController()
    private var menuController: StatusMenuController!
    private var desktopWindowController: DesktopWindowController!

    func applicationWillFinishLaunching(_ notification: Notification) {
        let currentPID = ProcessInfo.processInfo.processIdentifier
        let matchingApps = NSRunningApplication.runningApplications(withBundleIdentifier: Bundle.main.bundleIdentifier ?? "")
            .filter { $0.processIdentifier != currentPID && !$0.isTerminated }
        if let existingApp = matchingApps.first {
            existingApp.activate(options: [.activateAllWindows])
            NSApplication.shared.terminate(nil)
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.regular)
        notifier.requestAuthorization()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.isVisible = true
        if let button = statusItem.button {
            if let image = MenuBarIcon.makeImage() {
                button.title = ""
                button.image = image
                button.imagePosition = .imageOnly
                button.imageScaling = .scaleProportionallyDown
            } else {
                button.title = "★ Starlee"
                button.font = .systemFont(ofSize: NSFont.systemFontSize, weight: .semibold)
                button.contentTintColor = .labelColor
            }
            button.alignment = .center
            button.toolTip = "Starlee — click to save, Option-click for tools"
            button.setAccessibilityLabel("Starlee menu bar")
            button.setAccessibilityHelp("Click to save the current page. Option-click to open Starlee tools.")
        }
        NSLog("Starlee menu-bar status item created")

        menuController = StatusMenuController(
            statusItem: statusItem,
            client: client,
            notifier: notifier
        )
        menuController.rebuildMenu()

        desktopWindowController = DesktopWindowController(
            client: client,
            menuController: menuController
        )
        showDesktopWindow()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag {
            showDesktopWindow()
        }
        return true
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        if NSApplication.shared.windows.allSatisfy({ !$0.isVisible }) {
            showDesktopWindow()
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    private func showDesktopWindow() {
        desktopWindowController.showWindow(nil)
        NSApplication.shared.activate()
    }
}
