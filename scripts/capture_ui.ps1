# Captures the Aurelia main window (class `AureliaStudio`) to a PNG for UI review.
param(
    [string]$OutFile = 'C:\\Users\\Owner\\Documents\\Hollow Canvas\\ui-shot.png',
    [int]$WaitSeconds = 6
)

Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class WinCap {
    public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hWnd, StringBuilder sb, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter, int x, int y, int cx, int cy, uint flags);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hWnd, int attr, out RECT value, int size);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

    public static IntPtr FindByClass(uint pid, string cls) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint wpid; GetWindowThreadProcessId(h, out wpid);
            if (wpid != pid || !IsWindowVisible(h)) return true;
            var sb = new StringBuilder(256);
            GetClassNameW(h, sb, sb.Capacity);
            if (sb.ToString() == cls) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

$proc = Start-Process -FilePath 'C:\Users\Owner\Documents\Hollow Canvas\target\release\aurelia-app.exe' -PassThru

$hwnd = [IntPtr]::Zero
$deadline = (Get-Date).AddSeconds($WaitSeconds + 12)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    $proc.Refresh()
    if ($proc.HasExited) { Write-Error 'Aurelia exited before showing a window'; exit 1 }
    $hwnd = [WinCap]::FindByClass([uint32]$proc.Id, 'AureliaStudio')
    if ($hwnd -ne [IntPtr]::Zero) { break }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error 'Aurelia window not found'; Stop-Process -Id $proc.Id -Force; exit 1 }

[WinCap]::ShowWindow($hwnd, 9) | Out-Null   # SW_RESTORE
# HWND_TOPMOST + SWP_NOMOVE|SWP_NOSIZE|SWP_SHOWWINDOW: beats the foreground-request
# lock that otherwise leaves the terminal host painted over the app.
$SWP = 0x0001 -bor 0x0002 -bor 0x0040
[WinCap]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, $SWP) | Out-Null
[WinCap]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Seconds 2                      # let egui lay out and paint

$rect = New-Object WinCap+RECT
# DWMWA_EXTENDED_FRAME_BOUNDS (9) excludes the invisible resize border, so the
# desktop behind the window can't bleed into the capture.
$dwm = [WinCap]::DwmGetWindowAttribute($hwnd, 9, [ref]$rect, [System.Runtime.InteropServices.Marshal]::SizeOf($rect))
if ($dwm -ne 0) { [WinCap]::GetWindowRect($hwnd, [ref]$rect) | Out-Null }
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top

$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
$bmp.Save($OutFile, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

"captured $OutFile ($w x $h, $((Get-Item $OutFile).Length) bytes)"

Stop-Process -Id $proc.Id -Force
