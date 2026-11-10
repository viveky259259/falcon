import Foundation
import Vision
import ImageIO

for path in CommandLine.arguments.dropFirst() {
    let url = URL(fileURLWithPath: path)
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
        fatalError("Cannot decode image: \(path)")
    }
    let request = VNDetectBarcodesRequest()
    request.symbologies = [.qr]
    try VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
    let values = (request.results ?? []).compactMap { $0.payloadStringValue }
    let entry: [String: Any] = ["file": path, "payloads": values]
    let json = try JSONSerialization.data(withJSONObject: entry, options: [.sortedKeys])
    print(String(data: json, encoding: .utf8)!)
}
