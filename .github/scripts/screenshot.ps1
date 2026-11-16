# Launches the app, waits for its windows, and saves screenshots.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string] $OutDir
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Win {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hgt, uint flags);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[Win]::SetProcessDPIAware() | Out-Null

function Save-Region([int]$x, [int]$y, [int]$w, [int]$h, [string]$path) {
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "Saved $path ($w x $h)"
}

function Wait-Window([string]$title, [int]$seconds = 60) {
    for ($i = 0; $i -lt $seconds * 2; $i++) {
        $h = [Win]::FindWindow($null, $title)
        if ($h -ne [IntPtr]::Zero) { return $h }
        Start-Sleep -Milliseconds 500
    }
    throw "Window '$title' did not appear within $seconds s"
}

$proc = Start-Process -FilePath $Exe -PassThru
try {
    $dash = Wait-Window 'WPM Counter'
    Wait-Window 'WPM Counter Overlay' | Out-Null

    # Put the dashboard at a known spot, bring it forward, and let it render.
    [Win]::SetWindowPos($dash, [IntPtr]::Zero, 60, 60, 1080, 760, 0x0040) | Out-Null
    [Win]::SetForegroundWindow($dash) | Out-Null
    Start-Sleep -Seconds 6

    if ($proc.HasExited) { throw "App exited early with code $($proc.ExitCode)" }

    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    Save-Region $screen.X $screen.Y $screen.Width $screen.Height (Join-Path $OutDir 'desktop.png')

    $r = New-Object Win+RECT
    [Win]::GetWindowRect($dash, [ref]$r) | Out-Null
    Save-Region $r.Left $r.Top ($r.Right - $r.Left) ($r.Bottom - $r.Top) (Join-Path $OutDir 'dashboard.png')

    # The overlay sits in the top-right corner of the screen by default.
    $overlay = [Win]::FindWindow($null, 'WPM Counter Overlay')
    [Win]::GetWindowRect($overlay, [ref]$r) | Out-Null
    $pad = 16
    Save-Region ($r.Left - $pad) ($r.Top - $pad) ($r.Right - $r.Left + 2 * $pad) ($r.Bottom - $r.Top + 2 * $pad) (Join-Path $OutDir 'overlay.png')
}
finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
