import AppKit
import UserNotifications

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let client = StarleeClient()
    private let notifier = NotificationController()
    private let floatingButton = FloatingButtonController()
    private var menuController: StatusMenuController!

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.accessory)
        notifier.requestAuthorization()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.isVisible = true
        if let button = statusItem.button {
            button.title = "★ Starlee"
            button.font = .systemFont(ofSize: NSFont.systemFontSize, weight: .semibold)
            button.contentTintColor = .labelColor
            button.toolTip = "Starlee — save the current article"
            button.setAccessibilityLabel("Starlee menu bar")
        }
        NSLog("Starlee menu-bar status item created")

        menuController = StatusMenuController(
            statusItem: statusItem,
            client: client,
            notifier: notifier,
            floatingButton: floatingButton
        )
        floatingButton.show(target: menuController, action: #selector(StatusMenuController.saveCurrentArticle))
        menuController.rebuildMenu()
    }
}
