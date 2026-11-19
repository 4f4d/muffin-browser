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
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern IntPtr FindWindow(string className, string title);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetWindowRect(IntPtr window, out Rectangle rect);
}
'@
$process = Start-Process python -ArgumentList "spikes/s1/run_fixture.py $Binary --hold 12" -NoNewWindow -PassThru -RedirectStandardOutput "$Stem.txt" -RedirectStandardError "$Stem.stderr"
try {
    for ($sample = 0; $sample -lt 700 -and -not $process.HasExited; $sample++) {
        if (Select-String -Path "$Stem.txt" -Pattern '^screenshot_ready=true$' -Quiet) {
            $handle = [MuffinWindow]::FindWindow($null, $Title)
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
