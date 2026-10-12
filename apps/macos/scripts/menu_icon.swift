// 分别生成状态项 PNG 与系统输入源 PDF；小鸡等比居中，保留应用图与设置页完整字标。
import AppKit
import ImageIO
import UniformTypeIdentifiers

func alphaBounds(_ bitmap: NSBitmapImageRep) -> CGRect? {
    var left = bitmap.pixelsWide, top = bitmap.pixelsHigh
    var right = -1, bottom = -1
    for y in 0..<bitmap.pixelsHigh {
        for x in 0..<bitmap.pixelsWide {
            guard let color = bitmap.colorAt(x: x, y: y), color.alphaComponent > 1.0 / 255.0 else { continue }
            left = min(left, x); right = max(right, x)
            top = min(top, y); bottom = max(bottom, y)
        }
    }
    guard right >= left, bottom >= top else { return nil }
    return CGRect(x: left, y: top, width: right - left + 1, height: bottom - top + 1)
}

func generateMenuIcon(source: URL, output: URL, inputSourceOutput: URL?, templateOutput: URL?) throws {
    let data = try Data(contentsOf: source)
    guard let bitmap = NSBitmapImageRep(data: data), let sourceImage = bitmap.cgImage,
          let bounds = alphaBounds(bitmap), let cropped = sourceImage.cropping(to: bounds) else {
        throw NSError(domain: "VakyuMenuIcon", code: 1,
                      userInfo: [NSLocalizedDescriptionKey: "无法读取图标或图标完全透明"])
    }
    // PNG 的 144 dpi 明确对应 18pt；主体最长边 16pt，四周至少留 1pt。
    let pixels = 36
    let contentPixels: CGFloat = 32
    guard let context = CGContext(data: nil, width: pixels, height: pixels,
                                  bitsPerComponent: 8, bytesPerRow: pixels * 4,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
        throw NSError(domain: "VakyuMenuIcon", code: 2,
                      userInfo: [NSLocalizedDescriptionKey: "无法建立菜单图标画布"])
    }
    let scale = contentPixels / max(bounds.width, bounds.height)
    let width = bounds.width * scale, height = bounds.height * scale
    let rect = CGRect(x: (CGFloat(pixels) - width) / 2, y: (CGFloat(pixels) - height) / 2,
                      width: width, height: height)
    context.interpolationQuality = .high
    context.draw(cropped, in: rect)
    guard let image = context.makeImage(),
          let destination = CGImageDestinationCreateWithURL(output as CFURL, UTType.png.identifier as CFString, 1, nil) else {
        throw NSError(domain: "VakyuMenuIcon", code: 3,
                      userInfo: [NSLocalizedDescriptionKey: "无法写出菜单图标"])
    }
    CGImageDestinationAddImage(destination, image, [kCGImagePropertyDPIWidth: 144,
                                                  kCGImagePropertyDPIHeight: 144] as CFDictionary)
    guard CGImageDestinationFinalize(destination) else {
        throw NSError(domain: "VakyuMenuIcon", code: 4,
                      userInfo: [NSLocalizedDescriptionKey: "菜单图标保存失败"])
    }
    print("菜单图标：\(output.path)，36×36px / 18×18pt，主体等比居中")
    if let inputSourceOutput {
        try generateInputSourceIcon(cropped, output: inputSourceOutput)
    }
    if let templateOutput {
        try generateInputSourceIcon(templateImage(cropped), output: templateOutput)
    }
}

// 系统输入菜单会把图像着成单色：将原图的轮廓和五官变成透明底笔画，避免整块黄色底变成白方块。
func templateImage(_ image: CGImage) throws -> CGImage {
    let width = image.width, height = image.height
    let source = NSBitmapImageRep(cgImage: image)
    var alpha = [Double](repeating: 0, count: width * height)
    var features = alpha
    func unit(_ value: Double) -> Double { min(1, max(0, value)) }
    for y in 0..<height {
        for x in 0..<width {
            guard let color = source.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB) else { continue }
            let index = y * width + x
            alpha[index] = color.alphaComponent
            let dark = unit((0.60 - max(color.redComponent, color.greenComponent, color.blueComponent)) / 0.20)
            let light = unit((min(color.redComponent, color.greenComponent, color.blueComponent) - 0.78) / 0.16)
            features[index] = max(dark, light)
        }
    }
    let radius = max(1, Int(Double(max(width, height)) * 0.035))
    let directions = (0..<16).map { angle -> (Int, Int) in
        let radians = Double(angle) * .pi / 8
        return (Int((cos(radians) * Double(radius)).rounded()), Int((sin(radians) * Double(radius)).rounded()))
    }
    guard let result = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
                                        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                                        isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: width * 4,
                                        bitsPerPixel: 32), let bytes = result.bitmapData else {
        throw NSError(domain: "VakyuMenuIcon", code: 6)
    }
    for y in 0..<height {
        for x in 0..<width {
            let index = y * width + x
            let inner = directions.map { dx, dy -> Double in
                let xx = x + dx, yy = y + dy
                return xx >= 0 && xx < width && yy >= 0 && yy < height ? alpha[yy * width + xx] : 0
            }.min() ?? 0
            let opacity = alpha[index] * max(features[index], 1 - inner)
            bytes[index * 4] = 0; bytes[index * 4 + 1] = 0; bytes[index * 4 + 2] = 0
            bytes[index * 4 + 3] = UInt8((opacity * 255).rounded())
        }
    }
    guard let output = result.cgImage else { throw NSError(domain: "VakyuMenuIcon", code: 7) }
    return output
}

func generateInputSourceIcon(_ image: CGImage, output: URL) throws {
    // 系统输入源按文字基线布局：16pt 画布中主体留边，并上移 1pt 校正菜单栏的下沉；自有状态项 PNG 不变。
    var page = CGRect(x: 0, y: 0, width: 16, height: 16)
    guard let consumer = CGDataConsumer(url: output as CFURL),
          let context = CGContext(consumer: consumer, mediaBox: &page, nil) else {
        throw NSError(domain: "VakyuMenuIcon", code: 5,
                      userInfo: [NSLocalizedDescriptionKey: "无法建立输入源图标 PDF"])
    }
    let contentSize: CGFloat = 14
    let baselineCorrection: CGFloat = 1
    let scale = contentSize / CGFloat(max(image.width, image.height))
    let width = CGFloat(image.width) * scale, height = CGFloat(image.height) * scale
    let rect = CGRect(x: (page.width - width) / 2, y: (page.height - height) / 2 + baselineCorrection,
                      width: width, height: height)
    context.beginPDFPage(nil)
    context.draw(image, in: rect)
    context.endPDFPage()
    context.closePDF()
    print("输入源图标：\(output.path)，16×16pt PDF，14pt 主体，基线向上校正 1pt")
}

do {
    guard (3...5).contains(CommandLine.arguments.count) else {
        throw NSError(domain: "VakyuMenuIcon", code: 64,
                      userInfo: [NSLocalizedDescriptionKey: "用法：swift menu_icon.swift <原图.png> <状态项.png> [彩色输入源.pdf] [单色输入源Template.pdf]"])
    }
    try generateMenuIcon(source: URL(fileURLWithPath: CommandLine.arguments[1]),
                         output: URL(fileURLWithPath: CommandLine.arguments[2]),
                         inputSourceOutput: CommandLine.arguments.count >= 4
                             ? URL(fileURLWithPath: CommandLine.arguments[3]) : nil,
                         templateOutput: CommandLine.arguments.count == 5
                             ? URL(fileURLWithPath: CommandLine.arguments[4]) : nil)
} catch {
    FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
    exit(1)
}
