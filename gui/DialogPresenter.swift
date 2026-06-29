import AppKit
import UniformTypeIdentifiers

enum DialogPresenter {
    static func prompt(title: String, label: String) -> String? {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = label
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 360, height: 24))
        alert.accessoryView = field
        return alert.runModal() == .alertFirstButtonReturn ? field.stringValue : nil
    }

    static func show(title: String, message: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = String(message.prefix(5000))
        alert.runModal()
    }

    static func showTrace(title: String, summary: String, rawJSON: String) {
        let text = """
        \(summary)

        Raw trace JSON
        \(rawJSON)
        """
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = ""
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Copy")
        alert.addButton(withTitle: "Export")

        let textView = NSTextView(frame: NSRect(x: 0, y: 0, width: 680, height: 420))
        textView.isEditable = false
        textView.isSelectable = true
        textView.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        textView.string = text
        textView.textContainerInset = NSSize(width: 10, height: 10)

        let scrollView = NSScrollView(frame: textView.frame)
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = false
        scrollView.documentView = textView
        alert.accessoryView = scrollView

        let response = alert.runModal()
        if response == .alertSecondButtonReturn {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        } else if response == .alertThirdButtonReturn {
            let panel = NSSavePanel()
            panel.nameFieldStringValue = "starlee-last-capture-trace.json"
            panel.allowedContentTypes = [.json]
            if panel.runModal() == .OK, let url = panel.url {
                try? rawJSON.write(to: url, atomically: true, encoding: .utf8)
            }
        }
    }
}
