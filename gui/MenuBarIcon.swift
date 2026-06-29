import AppKit

enum MenuBarIcon {
    static let size = NSSize(width: 22, height: 22)

    // Pure constants — number of animation frames for each state.
    // Tested independently of AppKit rendering.
    static let loadingFrameCount = 3
    static let successFrameCount = 10

    // Progress value [0, 1] for a given success-frame index.
    static func pulseProgress(forIndex index: Int) -> CGFloat {
        guard successFrameCount > 1 else { return 0 }
        return CGFloat(index) / CGFloat(successFrameCount - 1)
    }

    static func makeImage() -> NSImage? {
        let image = NSImage(size: size)
        addRepresentation(named: "StarleeMenuBarIcon-22", to: image)
        addRepresentation(named: "StarleeMenuBarIcon-22@2x", to: image)
        guard !image.representations.isEmpty else { return nil }
        image.size = size
        image.isTemplate = false
        return image
    }

    static func loadingFrames() -> [NSImage] {
        guard let base = makeImage() else { return [] }
        return [
            stateImage(from: base, alpha: 0.54, tint: NSColor(calibratedWhite: 1, alpha: 0.06)),
            stateImage(from: base, alpha: 0.82, tint: NSColor(calibratedWhite: 1, alpha: 0.16)),
            stateImage(from: base, alpha: 1.0, tint: NSColor(calibratedWhite: 1, alpha: 0.08))
        ]
    }

    static func successFrames() -> [NSImage] {
        guard let base = makeImage() else { return [] }
        return (0..<10).map { index in
            let progress = CGFloat(index) / 9
            return pulseImage(from: base, progress: progress)
        }
    }

    static func errorImage() -> NSImage? {
        guard let base = makeImage() else { return nil }
        return stateImage(from: base, alpha: 1, tint: NSColor.systemRed.withAlphaComponent(0.34), drawsMark: true)
    }

    static func attentionImage() -> NSImage? {
        guard let base = makeImage() else { return nil }
        return stateImage(from: base, alpha: 1, tint: NSColor.systemOrange.withAlphaComponent(0.30), drawsAttentionMark: true)
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

    private static func stateImage(
        from base: NSImage,
        alpha: CGFloat,
        tint: NSColor,
        drawsMark: Bool = false,
        drawsAttentionMark: Bool = false
    ) -> NSImage {
        drawImage { rect in
            base.draw(in: rect, from: .zero, operation: .sourceOver, fraction: alpha)
            NSGraphicsContext.current?.compositingOperation = .sourceAtop
            tint.setFill()
            rect.fill()
            NSGraphicsContext.current?.compositingOperation = .sourceOver
            if drawsMark {
                drawErrorMark(in: rect)
            }
            if drawsAttentionMark {
                drawAttentionMark(in: rect)
            }
        }
    }

    private static func pulseImage(from base: NSImage, progress: CGFloat) -> NSImage {
        drawImage { rect in
            base.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1)
            let stripeWidth = rect.width * 0.82
            let x = rect.maxX - (rect.width + stripeWidth) * progress
            let stripeRect = NSRect(x: x, y: rect.minY - 2, width: stripeWidth, height: rect.height + 4)
            NSGraphicsContext.current?.compositingOperation = .sourceAtop
            NSGradient(colors: [
                NSColor(calibratedRed: 0.96, green: 0.73, blue: 0.44, alpha: 0),
                NSColor(calibratedRed: 1.0, green: 0.86, blue: 0.62, alpha: 0.58),
                NSColor(calibratedRed: 0.70, green: 0.93, blue: 1.0, alpha: 0.42),
                NSColor(calibratedRed: 1.0, green: 0.95, blue: 0.82, alpha: 0)
            ])?.draw(in: stripeRect, angle: 0)
            NSGraphicsContext.current?.compositingOperation = .sourceOver
        }
    }

    private static func drawImage(_ draw: (NSRect) -> Void) -> NSImage {
        let image = NSImage(size: size)
        image.lockFocus()
        NSGraphicsContext.current?.imageInterpolation = .high
        draw(NSRect(origin: .zero, size: size))
        image.unlockFocus()
        image.isTemplate = false
        return image
    }

    private static func drawErrorMark(in rect: NSRect) {
        let markRect = NSRect(x: rect.maxX - 8, y: rect.minY + 2, width: 6, height: 6)
        let path = NSBezierPath()
        path.lineWidth = 1.35
        path.lineCapStyle = .round
        NSColor(calibratedRed: 0.82, green: 0.18, blue: 0.16, alpha: 0.86).setStroke()
        path.move(to: NSPoint(x: markRect.minX, y: markRect.minY))
        path.line(to: NSPoint(x: markRect.maxX, y: markRect.maxY))
        path.move(to: NSPoint(x: markRect.maxX, y: markRect.minY))
        path.line(to: NSPoint(x: markRect.minX, y: markRect.maxY))
        path.stroke()
    }

    private static func drawAttentionMark(in rect: NSRect) {
        let markRect = NSRect(x: rect.maxX - 7, y: rect.minY + 2, width: 4, height: 8)
        let path = NSBezierPath()
        path.lineWidth = 1.5
        path.lineCapStyle = .round
        NSColor(calibratedRed: 0.78, green: 0.42, blue: 0.08, alpha: 0.92).setStroke()
        path.move(to: NSPoint(x: markRect.midX, y: markRect.maxY))
        path.line(to: NSPoint(x: markRect.midX, y: markRect.minY + 2.5))
        path.stroke()
        NSColor(calibratedRed: 0.78, green: 0.42, blue: 0.08, alpha: 0.92).setFill()
        NSBezierPath(ovalIn: NSRect(x: markRect.midX - 0.8, y: markRect.minY, width: 1.6, height: 1.6)).fill()
    }
}
