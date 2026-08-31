fn main() {
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=comdlg32");
    println!("cargo:rustc-link-lib=ole32");
    println!("cargo:rustc-link-lib=shell32");
    println!("cargo:rustc-link-lib=kernel32");
}
