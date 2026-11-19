$ErrorActionPreference = 'Stop'
$out = Join-Path $PWD 'm0-artifacts'
New-Item -ItemType Directory -Path $out -Force | Out-Null
$log = Join-Path $out 'engine-floor.txt'
$stage = Join-Path $env:RUNNER_TEMP 'muffin-floor'
New-Item -ItemType Directory -Path $stage -Force | Out-Null
$version = '1.0.4314.35'
$packages = Join-Path $stage 'packages'
& nuget install Microsoft.Web.WebView2 -Version $version -OutputDirectory $packages -NonInteractive
if ($LASTEXITCODE -ne 0) { throw 'WebView2 SDK download failed' }
$sdk = Join-Path $packages "Microsoft.Web.WebView2.$version\build\native"
$installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $installer -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'Visual Studio C++ build tools are unavailable' }
$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
$source = Join-Path $PSScriptRoot 'main.cpp'
$binary = Join-Path $out 'muffin-floor.exe'
$batch = Join-Path $stage 'compile.cmd'
@"
@echo off
call "$vcvars"
if errorlevel 1 exit /b 1
cd /d "$stage"
cl /nologo /std:c++17 /O2 /EHsc /I"$sdk\include" "$source" /Fe:"$binary" /link "$sdk\x64\WebView2LoaderStatic.lib" user32.lib ole32.lib shlwapi.lib version.lib advapi32.lib
"@ | Set-Content -Encoding ascii $batch
@(
    "source_commit=$env:GITHUB_SHA"
    "runner_image=$env:ImageOS/$env:ImageVersion"
    "os=$([System.Environment]::OSVersion.Version)"
    "architecture=$env:PROCESSOR_ARCHITECTURE"
    "webview2_sdk=$version"
) | Set-Content $log
& cmd.exe /d /c $batch 2>&1 | Tee-Object -FilePath $log -Append
if ($LASTEXITCODE -ne 0) { throw 'Native Windows engine floor did not compile' }
"binary_bytes=$((Get-Item $binary).Length)" | Add-Content $log
"binary_sha256=$((Get-FileHash -Algorithm SHA256 $binary).Hash.ToLower())" | Add-Content $log
$profile = Join-Path $stage 'profile'
& $binary $profile 2>&1 | Tee-Object -FilePath $log -Append
if ($LASTEXITCODE -ne 0) { throw 'Windows engine floor did not reach about:blank' }
