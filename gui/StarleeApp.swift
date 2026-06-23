import AppKit
import UserNotifications

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let client = StarleeClient()
    private let notifier = NotificationController()
    private var menuController: StatusMenuController!

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.accessory)
        notifier.requestAuthorization()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.isVisible = true
        if let button = statusItem.button {
            if let image = MenuBarIcon.makeImage() {
                button.title = ""
                button.image = image
                button.imagePosition = .imageOnly
            } else {
                button.title = "★ Starlee"
                button.font = .systemFont(ofSize: NSFont.systemFontSize, weight: .semibold)
                button.contentTintColor = .labelColor
            }
            button.toolTip = "Starlee — click to save, Option-click for tools"
            button.setAccessibilityLabel("Starlee menu bar")
        }
        NSLog("Starlee menu-bar status item created")

        menuController = StatusMenuController(
            statusItem: statusItem,
            client: client,
            notifier: notifier
        )
        menuController.rebuildMenu()
    }
}
