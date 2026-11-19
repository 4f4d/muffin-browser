import AppKit
import WebKit

final class Floor: NSObject, NSApplicationDelegate, WKNavigationDelegate {
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let rect = NSRect(x: 0, y: 0, width: 800, height: 600)
        let window = NSWindow(contentRect: rect, styleMask: [.titled, .closable, .resizable],
                              backing: .buffered, defer: false)
        window.title = "Muffin engine floor"
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        let webView = WKWebView(frame: rect, configuration: configuration)
        self.window = window
        window.contentView = webView
        webView.navigationDelegate = self
        window.center()
        window.orderFront(nil)
        webView.load(URLRequest(url: URL(string: "about:blank")!))
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        webView.evaluateJavaScript("({url: location.href, userAgent: navigator.userAgent})") { value, error in
            guard let value = value as? [String: String], value["url"] == "about:blank",
                  let bytes = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
                  let line = String(data: bytes, encoding: .utf8) else {
                FileHandle.standardError.write(Data("about:blank readiness probe failed: \(String(describing: error))\n".utf8))
                NSApplication.shared.terminate(nil)
                return
            }
            print(line)
            fflush(stdout)
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let floor = Floor()
app.delegate = floor
app.run()
