//CSIDL_ADMINTOOLS
//FOLDERID_AdminTools

use windows_sys::Win32::{Foundation::{GetLastError, MAX_PATH}, UI::Shell::SHGetSpecialFolderPathW};

const CSIDL_ADMINTOOLS: i32 = 48;
fn main() {
    unsafe {
        let mut out:Vec<u16> = vec![0;MAX_PATH as usize];
        out.set_len(MAX_PATH as usize);
        let status = SHGetSpecialFolderPathW(std::ptr::null_mut(),out.as_mut_ptr(),CSIDL_ADMINTOOLS,1);
        if status == 0 {
            println!("{:x}",GetLastError());
        } else {
            //'C:\Users\user\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Administrative Tools'
            let p = String::from_utf16_lossy(&out);
            println!("'{}'",&p);
        }
    }
}