// Headless macOS CoreGraphics PDF renderer; no AppKit, windows or installed app.
import Foundation
import CoreGraphics
import ImageIO
func render(_ input: URL, _ output: URL, _ scale: CGFloat) {
    let document = CGPDFDocument(input as CFURL)!
    let page = document.page(at: 1)!
    let box = page.getBoxRect(.mediaBox)
    let width = Int(ceil(box.width * scale)), height = Int(ceil(box.height * scale))
    let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                            bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.scaleBy(x: scale, y: scale)
    context.drawPDFPage(page)
    let image = context.makeImage()!
    let destination = CGImageDestinationCreateWithURL(output as CFURL, "public.png" as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, nil)
    precondition(CGImageDestinationFinalize(destination))
}
let args = CommandLine.arguments
let input = URL(fileURLWithPath: args[1]), output = URL(fileURLWithPath: args[2])
let scale = args.count > 3 ? CGFloat(Double(args[3])!) : 2
var directory: ObjCBool = false
precondition(FileManager.default.fileExists(atPath: input.path, isDirectory: &directory))
var count = 0
if directory.boolValue {
    try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
    let paths = try FileManager.default.contentsOfDirectory(at: input, includingPropertiesForKeys: nil)
        .filter { $0.pathExtension == "pdf" }.sorted { $0.path < $1.path }
    for path in paths {
        autoreleasepool { render(path, output.appendingPathComponent(path.deletingPathExtension().lastPathComponent + ".png"), scale) }
        count += 1
    }
} else { render(input, output, scale); count += 1 }
print("CoreGraphics PDF page 1 @\(scale)x / \(count) files")
