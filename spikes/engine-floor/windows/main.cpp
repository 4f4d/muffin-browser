#include <windows.h>
#include <WebView2.h>
#include <wrl.h>
#include <cstdio>
#include <cwchar>

using Microsoft::WRL::Callback;
using Microsoft::WRL::ComPtr;

static ComPtr<ICoreWebView2Controller> controller;
static ComPtr<ICoreWebView2> view;

static HRESULT check(const char *step, HRESULT code) {
    if (FAILED(code)) {
        std::fprintf(stderr, "%s: HRESULT 0x%08lX\n", step, static_cast<unsigned long>(code));
        PostQuitMessage(1);
    }
    return code;
}

static void resize(HWND window) {
    if (controller) {
        RECT bounds;
        GetClientRect(window, &bounds);
        controller->put_Bounds(bounds);
    }
}

static LRESULT CALLBACK window_proc(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    if (message == WM_SIZE) {
        resize(window);
    } else if (message == WM_TIMER) {
        std::fputs("WebView2 readiness watchdog expired\n", stderr);
        PostQuitMessage(1);
    } else if (message == WM_DESTROY) {
        PostQuitMessage(1);
    }
    return DefWindowProcW(window, message, wparam, lparam);
}

static HRESULT create_view(HWND window, ICoreWebView2Environment *environment) {
    PWSTR version = nullptr;
    HRESULT code = check("get_BrowserVersionString", environment->get_BrowserVersionString(&version));
    if (FAILED(code)) {
        return code;
    }
    std::printf("webview2=%ls\n", version);
    CoTaskMemFree(version);
    return environment->CreateCoreWebView2Controller(window,
        Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>(
            [window](HRESULT status, ICoreWebView2Controller *created) -> HRESULT {
                if (FAILED(check("controller callback", status))) {
                    return status;
                }
                controller = created;
                HRESULT code = check("get_CoreWebView2", created->get_CoreWebView2(&view));
                if (FAILED(code)) {
                    return code;
                }
                resize(window);
                EventRegistrationToken token{};
                code = check("add_NavigationCompleted", view->add_NavigationCompleted(
                    Callback<ICoreWebView2NavigationCompletedEventHandler>(
                        [](ICoreWebView2 *sender, ICoreWebView2NavigationCompletedEventArgs *args) -> HRESULT {
                            BOOL success = FALSE;
                            PWSTR source = nullptr;
                            args->get_IsSuccess(&success);
                            sender->get_Source(&source);
                            bool matched = success && source && std::wcscmp(source, L"about:blank") == 0;
                            std::printf("navigation_success=%d\nuri=%ls\n", success, source ? source : L"(missing)");
                            CoTaskMemFree(source);
                            PostQuitMessage(matched ? 0 : 1);
                            return S_OK;
                        }).Get(), &token));
                return FAILED(code) ? code : check("Navigate", view->Navigate(L"about:blank"));
            }).Get());
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 2 || FAILED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED))) {
        return 1;
    }
    HINSTANCE instance = GetModuleHandleW(nullptr);
    WNDCLASSW cls{};
    cls.lpfnWndProc = window_proc;
    cls.hInstance = instance;
    cls.lpszClassName = L"MuffinFloor";
    if (!RegisterClassW(&cls)) {
        return 1;
    }
    HWND window = CreateWindowExW(0, cls.lpszClassName, L"Muffin engine floor",
        WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, 800, 600, nullptr, nullptr, instance, nullptr);
    if (!window) {
        return 1;
    }
    ShowWindow(window, SW_SHOW);
    SetTimer(window, 1, 25000, nullptr);
    check("create environment", CreateCoreWebView2EnvironmentWithOptions(nullptr, argv[1], nullptr,
        Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>(
            [window](HRESULT status, ICoreWebView2Environment *environment) -> HRESULT {
                HRESULT code = check("environment callback", status);
                return FAILED(code) ? code : check("create controller", create_view(window, environment));
            }).Get()));
    MSG msg{};
    BOOL read;
    while ((read = GetMessageW(&msg, nullptr, 0, 0)) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    if (controller) {
        controller->Close();
    }
    view.Reset();
    controller.Reset();
    CoUninitialize();
    return read < 0 ? 1 : static_cast<int>(msg.wParam);
}
