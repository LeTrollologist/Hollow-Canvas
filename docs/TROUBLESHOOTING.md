# Troubleshooting

## Installer or launch

**SmartScreen warns about the installer.** It is not Authenticode-signed; verify the SHA-256 and
Ed25519 signature first, then choose *More info → Run anyway*.

**The app launches but cannot save anywhere.** Check that it is not running from a low-integrity
or sandboxed folder/process. Install it under `%LOCALAPPDATA%\Programs\Aurelia` or run it from a
normal Desktop folder; a low-integrity token cannot write Documents/Desktop. Also check Windows
Security → Ransomware protection → Controlled folder access.

## Discord Rich Presence

**No activity appears.** In order:

1. The app must be running; the activity disappears when it exits.
2. Discord → User Settings → Activity Privacy → **Share my activity** must be on.
3. In Activity Settings / Registered Games, the eye/toggle next to **Aurelia** must be visible.
4. The build must contain a Discord application ID. Without one, the menu says
   *“Discord Rich Presence (not configured)”* and Enable is disabled.
5. Discord must be the desktop client; the web client cannot use local IPC.

**Activity appears in recent activities but not on the profile.** That list is Discord’s game
detection and is independent of RPC. Check the per-game toggle and Activity Sharing above.

## Projects

**A project fails to open.** AUR v1/v2 and HCV v2–v5 are read-only compatibility formats; a
truncated or malformed file returns *Corrupt or unsupported .aur project data* rather than being
reinterpreted. Keep the file and report it with its size and version.

**Autosave exists but the app crashed.** Autosave snapshots live in
`%LOCALAPPDATA%\Aurelia\autosave`; open the `.aur` file directly. A recovery browser is not yet
implemented.

## Performance

Large canvases and deep undo histories are memory-bound. Close other documents, reduce undo
depth, or flatten layers before very large exports. See [PERFORMANCE.md](PERFORMANCE.md) for the
measured baseline.
