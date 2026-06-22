import AppKit

enum MenuBarIcon {
    static func makeImage() -> NSImage? {
        let image = NSImage(size: NSSize(width: 22, height: 22))
        addRepresentation(named: "StarleeMenuBarIcon-22", to: image)
        addRepresentation(named: "StarleeMenuBarIcon-22@2x", to: image)
        guard !image.representations.isEmpty else { return nil }
        image.size = NSSize(width: 22, height: 22)
        image.isTemplate = false
        return image
    }

    private static func addRepresentation(named name: String, to image: NSImage) {
        guard
            let url = Bundle.main.url(forResource: name, withExtension: "png"),
            let data = try? Data(contentsOf: url),
            let representation = NSBitmapImageRep(data: data)
        else {
            return
        }
        image.addRepresentation(representation)
    }
}
