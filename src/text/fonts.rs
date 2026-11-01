/// The system font directory for the current target platform.
#[cfg(target_os = "windows")]
pub const SYSTEM_FONTS_DIRECTORY: &str = "C:/Windows/Fonts";
/// The system font directory for the current target platform.
#[cfg(target_os = "linux")]
pub const SYSTEM_FONTS_DIRECTORY: &str = "/usr/share/fonts";
/// The system font directory for the current target platform.
#[cfg(target_os = "android")]
pub const SYSTEM_FONTS_DIRECTORY: &str = "/system/fonts";
/// The system font directory for the current target platform.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
pub const SYSTEM_FONTS_DIRECTORY: &str = "/System/Library/Fonts";
