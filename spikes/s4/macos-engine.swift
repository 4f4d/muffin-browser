import AppKit
import WebKit

final class Probe: NSObject, NSApplicationDelegate, WKNavigationDelegate {
    let root: URL
    let filtered: Bool
    let output: URL
    var view: WKWebView?
    let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 620, height: 450),
        styleMask: [.titled, .closable], backing: .buffered, defer: false
    )
    var phase = "pages"
    var navigationStarted = DispatchTime.now()

    init(root: URL, filtered: Bool, output: URL) {
        self.root = root
        self.filtered = filtered
        self.output = output
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 18) {
            print("engine_probe=timeout")
            exit(2)
        }
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        guard filtered else {
            launch(config)
            return
        }

        let store = WKContentRuleListStore(url: output.appendingPathComponent("rules"))!
        let rules = #"[{"trigger":{"url-filter":"/blocked[.]svg$"},"action":{"type":"block"}}]"#
        let began = DispatchTime.now()
        store.compileContentRuleList(forIdentifier: "muffin-test", encodedContentRuleList: rules) {
            rule, error in
            if let error {
                print("rule_compile_error=\(error)")
                exit(3)
            }
            guard let rule else { exit(3) }
            let millis = Double(DispatchTime.now().uptimeNanoseconds - began.uptimeNanoseconds) / 1e6
            print("rule_compiled=true elapsed_ms=\(millis)")
            config.userContentController.add(rule)
            self.launch(config)
        }
    }

    func launch(_ config: WKWebViewConfiguration) {
        let web = WKWebView(frame: window.contentView!.bounds, configuration: config)
        view = web
        web.navigationDelegate = self
        window.title = "Muffin S4 native engine probe"
        window.contentView = web
        window.center()
        window.makeKeyAndOrderFront(nil)
        navigationStarted = DispatchTime.now()
        web.load(URLRequest(url: root.appendingPathComponent("images")))
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        if phase == "pages" {
            phase = "images-finished"
            let millis = Double(DispatchTime.now().uptimeNanoseconds - navigationStarted.uptimeNanoseconds) / 1e6
            webView.evaluateJavaScript("({blocked:document.querySelector('#blocked').naturalWidth, control:document.querySelector('#control').naturalWidth})") { value, error in
                print("images_dom=\(String(describing: value)) drawing_error=\(String(describing: error)) page_finish_ms=\(millis)")
                if self.filtered {
                    self.phase = "pdf"
                    webView.load(URLRequest(url: self.root.appendingPathComponent("sample.pdf")))
                } else {
                    NSApp.terminate(nil)
                }
            }
        } else if phase == "pdf" {
            phase = "snapshot"
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                webView.takeSnapshot(with: nil) { image, error in
                    if let error { print("pdf_snapshot_error=\(error)") }
                    if let tiff = image?.tiffRepresentation,
                       let bitmap = NSBitmapImageRep(data: tiff),
                       let png = bitmap.representation(using: .png, properties: [:]) {
                        do {
                            try png.write(to: self.output.appendingPathComponent("pdf-view.png"))
                            print("pdf_snapshot=true width=\(bitmap.pixelsWide) height=\(bitmap.pixelsHigh)")
                        } catch {
                            print("pdf_snapshot_write_error=\(error)")
                        }
                    }
                    NSApp.terminate(nil)
                }
            }
        }
    }
}

let arguments = CommandLine.arguments
guard arguments.count == 4, let root = URL(string: arguments[1]),
      root.scheme == "http", root.host == "127.0.0.1", root.port != nil else {
    exit(1)
}
let app = NSApplication.shared
let probe = Probe(root: root, filtered: arguments[2] == "filtered",
                  output: URL(fileURLWithPath: arguments[3], isDirectory: true))
app.delegate = probe
app.setActivationPolicy(.regular)
app.run()
