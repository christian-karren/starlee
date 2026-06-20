import AppKit

final class FloatingButtonController {
    private var panel: NSPanel?

    func show(target: AnyObject, action: Selector) {
        if panel != nil {
            panel?.orderFrontRegardless()
            return
        }
        let size = NSSize(width: 52, height: 52)
        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1200, height: 800)
        let frame = NSRect(
            x: screen.maxX - size.width - 24,
            y: screen.maxY - size.height - 24,
            width: size.width,
            height: size.height
        )
        let panel = NSPanel(
            contentRect: frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.hidesOnDeactivate = false

        let button = NSButton(frame: NSRect(origin: .zero, size: size))
        button.title = "★"
        button.font = .systemFont(ofSize: 25, weight: .bold)
        button.bezelStyle = .circular
        button.target = target
        button.action = action
        button.toolTip = "Save current article to Starlee"
        panel.contentView = button
        panel.orderFrontRegardless()
        self.panel = panel
    }

    func hide() {
        panel?.close()
        panel = nil
    }
}
