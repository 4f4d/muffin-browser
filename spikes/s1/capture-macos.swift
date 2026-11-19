import CoreGraphics
import Foundation

let title = CommandLine.arguments[1]
let destination = CommandLine.arguments[2]
let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID)
let matching = (windows as? [[String: Any]] ?? []).filter { $0[kCGWindowName as String] as? String == title }
guard matching.count == 1, let id = matching[0][kCGWindowNumber as String] as? Int else {
    FileHandle.standardError.write(Data("Expected exactly one on-screen spike window with that title.\n".utf8))
    exit(1)
}
let capture = Process()
capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
capture.arguments = ["-x", "-o", "-l", String(id), destination]
try capture.run()
capture.waitUntilExit()
exit(capture.terminationStatus)
