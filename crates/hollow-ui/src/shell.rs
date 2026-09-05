#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".jfif", ".gif", ".webp", ".bmp", ".ico", ".tga", ".tiff", ".hcv",
];

/// Registers Hollow Canvas in the Windows Explorer context menu ("Open with Hollow Canvas")
/// for all supported image formats and .hcv project files under HKCU (no admin rights needed).
pub fn register_shell_associations() -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows context menu registration is only supported on Windows.".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get exe path: {}", e))?;
        let exe_str = exe_path.to_string_lossy();
        let cmd_str = format!("\"{}\" \"%1\"", exe_str);
        let icon_str = format!("\"{}\",0", exe_str);

        // 1. Register SystemFileAssociations for standard image extensions
        for &ext in SUPPORTED_EXTENSIONS {
            let base_key = format!(r"HKCU\Software\Classes\SystemFileAssociations\{}\shell\OpenWithHollowCanvas", ext);
            let cmd_key = format!(r"{}\command", base_key);

            // Set display name
            let status = Command::new("reg")
                .args(&["add", &base_key, "/ve", "/d", "Open with Hollow Canvas", "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|e| format!("Failed to run reg.exe: {}", e))?;

            if !status.success() {
                return Err(format!("Failed to register context menu for {}", ext));
            }

            // Set icon
            let _ = Command::new("reg")
                .args(&["add", &base_key, "/v", "Icon", "/d", &icon_str, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();

            // Set command
            let status_cmd = Command::new("reg")
                .args(&["add", &cmd_key, "/ve", "/d", &cmd_str, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|_e| format!("Failed to register command for {}", ext))?;

            if !status_cmd.success() {
                return Err(format!("Failed to register launch command for {}", ext));
            }
        }

        // 2. Register dedicated .hcv project association
        let hcv_ext_key = r"HKCU\Software\Classes\.hcv";
        let _ = Command::new("reg")
            .args(&["add", hcv_ext_key, "/ve", "/d", "HollowCanvas.Project", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        let progid_key = r"HKCU\Software\Classes\HollowCanvas.Project";
        let _ = Command::new("reg")
            .args(&["add", progid_key, "/ve", "/d", "Hollow Canvas Project File", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        let progid_icon = format!(r"{}\DefaultIcon", progid_key);
        let _ = Command::new("reg")
            .args(&["add", &progid_icon, "/ve", "/d", &icon_str, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        let progid_cmd = format!(r"{}\shell\open\command", progid_key);
        let _ = Command::new("reg")
            .args(&["add", &progid_cmd, "/ve", "/d", &cmd_str, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        Ok(format!(
            "Successfully registered 'Open with Hollow Canvas' for {} file extensions (.png, .jpg, .gif, .webp, .bmp, .hcv, etc.)!",
            SUPPORTED_EXTENSIONS.len()
        ))
    }
}

/// Unregisters Hollow Canvas context menu associations from HKCU.
pub fn unregister_shell_associations() -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows context menu registration is only supported on Windows.".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        for &ext in SUPPORTED_EXTENSIONS {
            let base_key = format!(r"HKCU\Software\Classes\SystemFileAssociations\{}\shell\OpenWithHollowCanvas", ext);
            let _ = Command::new("reg")
                .args(&["delete", &base_key, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }

        let _ = Command::new("reg")
            .args(&["delete", r"HKCU\Software\Classes\HollowCanvas.Project", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        let _ = Command::new("reg")
            .args(&["delete", r"HKCU\Software\Classes\.hcv", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();

        Ok("Successfully removed Hollow Canvas from Windows Explorer context menu.".to_string())
    }
}

/// Checks if Hollow Canvas is currently registered in HKCU context menu.
pub fn is_shell_registered() -> bool {
    #[cfg(not(target_os = "windows"))]
    {
        false
    }

    #[cfg(target_os = "windows")]
    {
        let check_key = r"HKCU\Software\Classes\SystemFileAssociations\.png\shell\OpenWithHollowCanvas";
        let output = Command::new("reg")
            .args(&["query", check_key])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }
}
