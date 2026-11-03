# Installing Aurelia

Aurelia runs on Windows 10/11 (64-bit) and needs no runtime, no WebView and no
administrator rights. Pick either distribution:

## Option 1 — Installer (recommended)

1. Download **`Aurelia-Setup-<version>.exe`** from the [latest release](https://github.com/LeTrollologist/Aurelia/releases/latest).
2. Run it. It installs **per user** into `%LOCALAPPDATA%\Programs\Aurelia`, so no UAC prompt appears.
3. Leave *"Open Aurelia projects with Aurelia (.aur, .hcv)"* checked if you want double-click
   opening and thumbnails/icon wiring for project files (optional).
4. Optionally tick *Create a desktop shortcut*, then **Install**.

Silent install (e.g. for scripted setups):

```powershell
Aurelia-Setup-2.4.1.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
```

Uninstalling (Settings → Apps, or the Start-menu entry) removes the program folder and
**only** clears the `.aur`/`.hcv` association if it still points at Aurelia — if you have since
chosen another editor for those files, your choice is left alone.

## Option 2 — Portable zip

No installer, no registry writes except whatever you trigger yourself:

```powershell
Expand-Archive -Path .\Aurelia-v2.4.1-win64.zip -DestinationPath .\Aurelia
.\Aurelia\aurelia.exe
```

The folder contains `aurelia.exe` plus the documentation and licence. Settings are stored in
`%LOCALAPPDATA%\Aurelia` either way, so the portable build remembers your theme, recent
projects and autosave interval.

## Optional: Explorer context menu

The installer wires double-click opening. To also get **"Open with Aurelia"** on image files
(`.png`, `.jpg`, `.gif`, `.webp`, `.bmp`, `.ico`, `.tga`, `.tiff`) and projects:

```powershell
.\aurelia.exe --register-shell     # add the context-menu entry (HKCU only)
.\aurelia.exe --unregister-shell   # remove it again
```

## Verifying a download

Every release ships `SHA256SUMS.txt`:

```powershell
Get-FileHash .\Aurelia-Setup-2.4.1.exe -Algorithm SHA256
# compare against the matching line in SHA256SUMS.txt
```

The zip and the installer are also submitted to VirusTotal by the release pipeline; the
release notes list the per-artifact verdict and a permalink, and `virustotal-summary.txt`
ships with the download.

## Building from source

```powershell
git clone https://github.com/LeTrollologist/Aurelia.git
cd Aurelia
cargo build --release -p aurelia-app   # -> target\release\aurelia-app.exe
```

Or produce the same artifacts the release pipeline creates:

```powershell
make check    # build + test + package + verify, nothing published
```
