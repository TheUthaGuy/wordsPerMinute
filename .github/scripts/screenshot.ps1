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
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
    public static void Click(IntPtr h, int cx, int cy) {
        var p = new POINT { X = cx, Y = cy };
        ClientToScreen(h, ref p);
        SetCursorPos(p.X, p.Y);
        System.Threading.Thread.Sleep(200);
        mouse_event(0x2, 0, 0, 0, UIntPtr.Zero); // left down
        System.Threading.Thread.Sleep(80);
        mouse_event(0x4, 0, 0, 0, UIntPtr.Zero); // left up
        System.Threading.Thread.Sleep(200);
        SetCursorPos(0, 0);
    }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
        public int dmFields, dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
        public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public short dmLogPixels;
        public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
        public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool EnumDisplaySettings(string dev, int mode, ref DEVMODE dm);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int ChangeDisplaySettings(ref DEVMODE dm, int flags);
    public static int SetResolution(int w, int h) {
        var dm = new DEVMODE(); dm.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE));
        if (!EnumDisplaySettings(null, -1, ref dm)) return -99;
        dm.dmPelsWidth = w; dm.dmPelsHeight = h; dm.dmFields = 0x80000 | 0x100000;
        return ChangeDisplaySettings(ref dm, 0);
    }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    public static System.Collections.Generic.List<string> WindowsOf(uint pid) {
        var list = new System.Collections.Generic.List<string>();
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid) {
                var sb = new System.Text.StringBuilder(256); GetWindowText(h, sb, 256);
                list.Add(string.Format("0x{0:X} visible={1} title='{2}'", h.ToInt64(), IsWindowVisible(h), sb));
            }
            return true;
        }, IntPtr.Zero);
        return list;
    }
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
        # [NullString]::Value: PowerShell would turn $null into "" here.
        $h = [Win]::FindWindow([NullString]::Value, $title)
        if ($h -ne [IntPtr]::Zero) { return $h }
        Start-Sleep -Milliseconds 500
    }
    throw "Window '$title' did not appear within $seconds s"
}

# The runner's screen is 1024x768, too small for the dashboard. Try to enlarge it.
$res = [Win]::SetResolution(1600, 900)
Write-Host "ChangeDisplaySettings(1600x900) returned $res (0 = success)"
# Minimise the runner's own console so it isn't behind the app.
(New-Object -ComObject Shell.Application).MinimizeAll()
Start-Sleep -Seconds 2

$env:RUST_BACKTRACE = '1'
$env:RUST_LOG = 'warn'
$logOut = Join-Path $OutDir 'app-stdout.log'
$logErr = Join-Path $OutDir 'app-stderr.log'
$proc = Start-Process -FilePath $Exe -PassThru -WorkingDirectory (Split-Path $Exe) `
    -RedirectStandardOutput $logOut -RedirectStandardError $logErr
try {
    $dash = Wait-Window 'WPM Counter'
    Wait-Window 'WPM Counter Overlay' | Out-Null

    # Put the dashboard at a known spot, bring it forward, and let it render.
    [Win]::SetWindowPos($dash, [IntPtr]::Zero, 40, 40, 1100, 800, 0x0040) | Out-Null
    [Win]::SetForegroundWindow($dash) | Out-Null
    Start-Sleep -Seconds 6

    if ($proc.HasExited) { throw "App exited early with code $($proc.ExitCode)" }

    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    Save-Region $screen.X $screen.Y $screen.Width $screen.Height (Join-Path $OutDir 'desktop.png')

    $r = New-Object Win+RECT
    [Win]::GetWindowRect($dash, [ref]$r) | Out-Null
    Save-Region $r.Left $r.Top ($r.Right - $r.Left) ($r.Bottom - $r.Top) (Join-Path $OutDir 'dashboard.png')

    # The overlay sits in the top-right corner of the screen by default.
    $overlay = [Win]::FindWindow([NullString]::Value, 'WPM Counter Overlay')
    [Win]::GetWindowRect($overlay, [ref]$r) | Out-Null
    $pad = 16
    Save-Region ($r.Left - $pad) ($r.Top - $pad) ($r.Right - $r.Left + 2 * $pad) ($r.Bottom - $r.Top + 2 * $pad) (Join-Path $OutDir 'overlay.png')

    # Open the "Past sessions" tab (client coords of the tab button) and capture it.
    [Win]::SetForegroundWindow($dash) | Out-Null
    [Win]::Click($dash, 335, 23)
    Start-Sleep -Seconds 3
    [Win]::GetWindowRect($dash, [ref]$r) | Out-Null
    Save-Region $r.Left $r.Top ($r.Right - $r.Left) ($r.Bottom - $r.Top) (Join-Path $OutDir 'history.png')
}
catch {
    Write-Host "::group::Diagnostics"
    Write-Host "Process exited: $($proc.HasExited)"
    if ($proc.HasExited) { Write-Host "Exit code: $($proc.ExitCode)" }
    else { [Win]::WindowsOf([uint32]$proc.Id) | ForEach-Object { Write-Host "  window $_" } }
    Write-Host "--- stderr ---"; Get-Content $logErr -ErrorAction SilentlyContinue
    Write-Host "--- stdout ---"; Get-Content $logOut -ErrorAction SilentlyContinue
    Write-Host "--- DLLs next to exe ---"; Get-ChildItem (Split-Path $Exe) -Filter *.dll | ForEach-Object Name
    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    Save-Region $screen.X $screen.Y $screen.Width $screen.Height (Join-Path $OutDir 'failure-desktop.png')
    Write-Host "::endgroup::"
    throw
}
finally {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
