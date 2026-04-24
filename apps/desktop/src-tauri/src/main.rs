// Keep the desktop build GUI-only when launched from Explorer on Windows.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
 unvet_desktop_lib::run();
}
