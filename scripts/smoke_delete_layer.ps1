# End-to-end regression check for the v2.4.0 "delete layer" crash.
#
# Drives the real app: adds two layers through the layer panel, deletes the top one
# with its trash button (twice, so the shrinking list is exercised again) and reports
# whether the app survived, capturing a screenshot at each step.
#
# Clicks are posted straight to the window (WM_MOUSEMOVE/LBUTTONDOWN/LBUTTONUP) so an
# overlapping console or terminal cannot swallow them. Coordinates are client-relative.
param(
    [string]$OutDir = 'C:\Users\Owner\Documents\Hollow Canvas',
    [int]$AddX = 1149, [int]$AddY = 547,
    [int]$TrashX = 1326, [int]$TrashY = 587
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class UiSmoke {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder sb, int n);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

    static IntPtr Point(int x, int y) { return (IntPtr)((y << 16) | (x & 0xFFFF)); }

    /// Posts a full click at client coordinates; occlusion by other windows is irrelevant.
    public static void Click(IntPtr hwnd, int x, int y) {
        PostMessage(hwnd, 0x0200, IntPtr.Zero, Point(x, y));   // WM_MOUSEMOVE
        System.Threading.Thread.Sleep(60);
        PostMessage(hwnd, 0x0201, (IntPtr)1, Point(x, y));     // WM_LBUTTONDOWN
        System.Threading.Thread.Sleep(90);
        PostMessage(hwnd, 0x0202, IntPtr.Zero, Point(x, y));   // WM_LBUTTONUP
    }

    public static IntPtr ByClass(uint pid, string cls) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid || !IsWindowVisible(h)) return true;
            var sb = new StringBuilder(256); GetClassNameW(h, sb, sb.Capacity);
            if (sb.ToString() == cls) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

Get-Process aurelia-app -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
$autosave = Join-Path $env:LOCALAPPDATA 'Aurelia\autosave'
if (Test-Path $autosave) { Remove-Item (Join-Path $autosave '*') -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 500

$exe = 'C:\Users\Owner\Documents\Hollow Canvas\target\release\aurelia-app.exe'
# Minimised keeps the app's console off the screen so screenshots show the panel.
# CreateNoWindow: the app's console never appears, so it cannot cover the UI.
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.UseShellExecute = $false
$psi.CreateNoWindow = $true
$proc = [System.Diagnostics.Process]::Start($psi)
$hwnd = [IntPtr]::Zero
$deadline = (Get-Date).AddSeconds(20)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 400
    if ($proc.HasExited) { Write-Error "app exited during startup ($($proc.ExitCode))"; exit 1 }
    $hwnd = [UiSmoke]::ByClass([uint32]$proc.Id, 'AureliaStudio')
    if ($hwnd -ne [IntPtr]::Zero) { break }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error 'window not found'; Stop-Process -Id $proc.Id -Force; exit 1 }
Start-Sleep -Seconds 2

function Shot([string]$name) {
    [UiSmoke]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0003) | Out-Null  # TOPMOST, no move/size
    [UiSmoke]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 350
    $proc.Refresh()
    if ($proc.HasExited) { "app died before '$name'"; return }
    $r = New-Object UiSmoke+RECT
    [UiSmoke]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $bmp.Save((Join-Path $OutDir $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    "captured $name"
}

Shot 'ui-del-1-before.png'

[UiSmoke]::Click($hwnd, $AddX, $AddY); Start-Sleep -Milliseconds 350
[UiSmoke]::Click($hwnd, $AddX, $AddY); Start-Sleep -Milliseconds 800
Shot 'ui-del-2-added.png'

[UiSmoke]::Click($hwnd, $TrashX, $TrashY); Start-Sleep -Milliseconds 900
$proc.Refresh(); $alive1 = -not $proc.HasExited
Shot 'ui-del-3-after.png'
"alive after delete #1: $alive1"

[UiSmoke]::Click($hwnd, $TrashX, $TrashY); Start-Sleep -Milliseconds 900
$proc.Refresh(); $alive2 = -not $proc.HasExited
"alive after delete #2: $alive2"

Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
if (Test-Path $autosave) { Remove-Item (Join-Path $autosave '*') -Force -ErrorAction SilentlyContinue }
if (-not ($alive1 -and $alive2)) { exit 2 }
