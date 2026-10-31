#[cfg(test)]

mod window_tests {
    use std::iter;

use vnc_module::{set_desktop_helper, windows};

    #[test]
    fn enum_windows_with_affinity_desktop_test() {
        unsafe {
            let desktop_name:Vec<u16> = "hdesktop1".encode_utf16().chain(iter::once(0)).collect();
            set_desktop_helper(desktop_name);
            
            let all_windows = windows::windows_with_affinity_info();
            for window in all_windows {
                println!("{:?} {:?} {:?} {:x?}",window.window_name,window.module_path,window.minimized,window.affinity);
            }
        }
    }

        #[test]
    fn enum_windows_desktop_test() {
        unsafe {
            let desktop_name:Vec<u16> = "hdesktop1".encode_utf16().chain(iter::once(0)).collect();
            set_desktop_helper(desktop_name);
            
            let all_windows = windows::windows();
            for window in all_windows {
                println!("{:?} {:?} {:?}",window.window_name,window.module_path,window.minimized);
            }
        }
    }
}