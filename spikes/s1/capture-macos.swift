import AppKit
import CoreGraphics
import Foundation

let arguments = CommandLine.arguments
guard arguments.count >= 3, let pid = Int32(arguments[2]), pid > 0 else {
    fputs("usage: capture-macos activate PID | capture PID PATH\n", stderr)
    exit(2)
}
if arguments[1] == "activate" {
    guard let application = NSRunningApplication(processIdentifier: pid) else {
        fputs("fixture is unavailable to NSRunningApplication\n", stderr)
        exit(1)
    }
    exit(application.activate() ? 0 : 1)
}
guard arguments[1] == "capture", arguments.count == 4 else { exit(2) }
let windows = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID)
let matching = (windows as? [[String: Any]] ?? []).filter {
    $0[kCGWindowOwnerPID as String] as? Int32 == pid && $0[kCGWindowLayer as String] as? Int == 0
}
guard matching.count == 1, let id = matching[0][kCGWindowNumber as String] as? Int else {
    fputs("Expected exactly one on-screen layer-0 window for the fixture process.\n", stderr)
    exit(1)
}
let capture = Process()
capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
capture.arguments = ["-x", "-o", "-l", String(id), arguments[3]]
try capture.run()
capture.waitUntilExit()
exit(capture.terminationStatus)
