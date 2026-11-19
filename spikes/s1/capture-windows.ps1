param (
    [Parameter(Mandatory)][string]$Binary,
    [Parameter(Mandatory)][string]$Title,
    [Parameter(Mandatory)][string]$Stem
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MuffinWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rectangle {
        public int Left, Top, Right, Bottom;
    }
    private delegate bool WindowCallback(IntPtr hwnd, IntPtr extra);
    [DllImport("user32.dll")] private static extern bool EnumWindows(WindowCallback callback, IntPtr extra);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    public static IntPtr FindForProcess(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd, extra) => {
            uint owner;
            GetWindowThreadProcessId(hwnd, out owner);
            Rectangle rect;
            if (owner == pid && IsWindowVisible(hwnd) && GetWindowRect(hwnd, out rect) &&
                rect.Right - rect.Left >= 720 && rect.Bottom - rect.Top >= 520) {
                found = hwnd;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetWindowRect(IntPtr window, out Rectangle rect);
}
'@
$process = Start-Process python -ArgumentList "spikes/s1/run_fixture.py $Binary --hold 12" -NoNewWindow -PassThru -RedirectStandardOutput "$Stem.txt" -RedirectStandardError "$Stem.stderr"
try {
    for ($sample = 0; $sample -lt 700 -and -not $process.HasExited; $sample++) {
        if (Select-String -Path "$Stem.txt" -Pattern '^screenshot_ready=true$' -Quiet) {
            $pidEntry = Select-String -Path "$Stem.txt" -Pattern '^fixture_app_pid=(\d+)$' | Select-Object -First 1
            if (!$pidEntry) { throw "Missing fixture PID for $Title" }
            $fixturePid = [uint32]$pidEntry.Matches[0].Groups[1].Value
            $handle = [MuffinWindow]::FindForProcess($fixturePid)
            $rect = New-Object MuffinWindow+Rectangle
            if ($handle -ne [IntPtr]::Zero -and [MuffinWindow]::GetWindowRect($handle, [ref]$rect)) {
                $width = $rect.Right - $rect.Left
                $height = $rect.Bottom - $rect.Top
                $bitmap = [Drawing.Bitmap]::new($width, $height)
                $graphics = [Drawing.Graphics]::FromImage($bitmap)
                try {
                    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
                    $bitmap.Save("$Stem.png", [Drawing.Imaging.ImageFormat]::Png)
                } finally {
                    $graphics.Dispose()
                    $bitmap.Dispose()
                }
            } else {
                Write-Warning 'S1 window could not be located for capture'
            }
            break
        }
        Start-Sleep -Milliseconds 250
    }
} catch {
    Write-Warning "S1 capture failed: $_"
} finally {
    $process.WaitForExit()
}
if ($process.ExitCode -ne 0) { exit $process.ExitCode }
