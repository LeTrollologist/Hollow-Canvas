use std::path::PathBuf;

#[cfg(target_os = "windows")]
mod win32 {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::PathBuf;

    #[repr(C)]
    struct OPENFILENAMEW {
        l_struct_size: u32,
        hwnd_owner: *mut std::ffi::c_void,
        h_instance: *mut std::ffi::c_void,
        lpstr_filter: *const u16,
        lpstr_custom_filter: *mut u16,
        n_max_cust_filter: u32,
        n_filter_index: u32,
        lpstr_file: *mut u16,
        n_max_file: u32,
        lpstr_file_title: *mut u16,
        n_max_file_title: u32,
        lpstr_initial_dir: *const u16,
        lpstr_title: *const u16,
        flags: u32,
        n_file_offset: u16,
        n_file_extension: u16,
        lpstr_def_ext: *const u16,
        l_cust_data: usize,
        lpfn_hook: *mut std::ffi::c_void,
        lp_template_name: *const u16,
        pv_reserved: *mut std::ffi::c_void,
        dw_reserved: u32,
        flags_ex: u32,
    }

    type FnGetOpenFileNameW = unsafe extern "system" fn(*mut OPENFILENAMEW) -> i32;
    type FnGetSaveFileNameW = unsafe extern "system" fn(*mut OPENFILENAMEW) -> i32;

    extern "system" {
        fn LoadLibraryA(lpLibFileName: *const u8) -> *mut std::ffi::c_void;
        fn GetProcAddress(hModule: *mut std::ffi::c_void, lpProcName: *const u8) -> *mut std::ffi::c_void;
    }

    const OFN_EXPLORER: u32 = 0x00080000;
    const OFN_FILEMUSTEXIST: u32 = 0x00001000;
    const OFN_PATHMUSTEXIST: u32 = 0x00000800;
    const OFN_OVERWRITEPROMPT: u32 = 0x00000002;

    fn encode_wide_filter(description: &str, pattern: &str) -> Vec<u16> {
        let mut out = Vec::new();
        out.extend(std::ffi::OsStr::new(description).encode_wide());
        out.push(0);
        out.extend(std::ffi::OsStr::new(pattern).encode_wide());
        out.push(0);
        out.push(0);
        out
    }

    pub fn pick_open_file(title: &str, filter_desc: &str, filter_pat: &str, def_ext: &str) -> Option<PathBuf> {
        unsafe {
            let lib = LoadLibraryA(b"comdlg32.dll\0".as_ptr());
            if lib.is_null() {
                return None;
            }
            let func_ptr = GetProcAddress(lib, b"GetOpenFileNameW\0".as_ptr());
            if func_ptr.is_null() {
                return None;
            }
            let get_open_file: FnGetOpenFileNameW = std::mem::transmute(func_ptr);

            let mut file_buf = [0u16; 2048];
            let filter = encode_wide_filter(filter_desc, filter_pat);
            let title_wide: Vec<u16> = std::ffi::OsStr::new(title).encode_wide().chain(Some(0)).collect();
            let def_ext_wide: Vec<u16> = std::ffi::OsStr::new(def_ext).encode_wide().chain(Some(0)).collect();

            let mut ofn: OPENFILENAMEW = std::mem::zeroed();
            ofn.l_struct_size = std::mem::size_of::<OPENFILENAMEW>() as u32;
            ofn.lpstr_filter = filter.as_ptr();
            ofn.lpstr_file = file_buf.as_mut_ptr();
            ofn.n_max_file = file_buf.len() as u32;
            ofn.lpstr_title = title_wide.as_ptr();
            ofn.lpstr_def_ext = def_ext_wide.as_ptr();
            ofn.flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;

            if get_open_file(&mut ofn) != 0 {
                let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
                let os_string = OsString::from_wide(&file_buf[..len]);
                return Some(PathBuf::from(os_string));
            }
        }
        None
    }

    pub fn pick_save_file(title: &str, filter_desc: &str, filter_pat: &str, def_ext: &str, default_filename: &str) -> Option<PathBuf> {
        unsafe {
            let lib = LoadLibraryA(b"comdlg32.dll\0".as_ptr());
            if lib.is_null() {
                return None;
            }
            let func_ptr = GetProcAddress(lib, b"GetSaveFileNameW\0".as_ptr());
            if func_ptr.is_null() {
                return None;
            }
            let get_save_file: FnGetSaveFileNameW = std::mem::transmute(func_ptr);

            let mut file_buf = [0u16; 2048];
            let default_wide: Vec<u16> = std::ffi::OsStr::new(default_filename).encode_wide().collect();
            let copy_len = default_wide.len().min(file_buf.len() - 1);
            file_buf[..copy_len].copy_from_slice(&default_wide[..copy_len]);

            let filter = encode_wide_filter(filter_desc, filter_pat);
            let title_wide: Vec<u16> = std::ffi::OsStr::new(title).encode_wide().chain(Some(0)).collect();
            let def_ext_wide: Vec<u16> = std::ffi::OsStr::new(def_ext).encode_wide().chain(Some(0)).collect();

            let mut ofn: OPENFILENAMEW = std::mem::zeroed();
            ofn.l_struct_size = std::mem::size_of::<OPENFILENAMEW>() as u32;
            ofn.lpstr_filter = filter.as_ptr();
            ofn.lpstr_file = file_buf.as_mut_ptr();
            ofn.n_max_file = file_buf.len() as u32;
            ofn.lpstr_title = title_wide.as_ptr();
            ofn.lpstr_def_ext = def_ext_wide.as_ptr();
            ofn.flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;

            if get_save_file(&mut ofn) != 0 {
                let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
                let os_string = OsString::from_wide(&file_buf[..len]);
                return Some(PathBuf::from(os_string));
            }
        }
        None
    }
}

pub fn open_project_dialog() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        win32::pick_open_file("Open Hollow Canvas Project", "Hollow Canvas Project (*.hcv)", "*.hcv", "hcv")
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

pub fn save_project_dialog() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        win32::pick_save_file("Save Hollow Canvas Project", "Hollow Canvas Project (*.hcv)", "*.hcv", "hcv", "artwork.hcv")
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

pub fn export_png_dialog() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        win32::pick_save_file("Export Flat PNG Image", "PNG Image (*.png)", "*.png", "png", "artwork.png")
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

pub fn open_image_dialog() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        win32::pick_open_file("Open Reference Image", "Image Files (*.png;*.jpg;*.jpeg;*.webp)\0*.png;*.jpg;*.jpeg;*.webp\0All Files (*.*)", "*.*", "png")
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
