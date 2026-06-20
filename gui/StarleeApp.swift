import AppKit
import UserNotifications

@main
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private let client = StarleeClient()
    private let notifier = NotificationController()
    private let floatingButton = FloatingButtonController()
    private var menuController: StatusMenuController!

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.accessory)
        notifier.requestAuthorization()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.title = "★"
        statusItem.button?.toolTip = "Starlee"

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
