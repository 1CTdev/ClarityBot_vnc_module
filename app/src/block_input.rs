use core::time;

use vnc_module::{input::{block_input_wrapper, hide_cursor}, monitors::disable_monitors};
use windows_sys::Win32::Foundation::GetLastError;

//works but needs admin
fn main() {
    unsafe {
        let status = block_input_wrapper(1);
        println!("{:x}",status);
        if status == 0 {
            let status = GetLastError();
            println!("{:x}",status);
        }

        
        let status = disable_monitors(1);
        println!("{:x}",status);
        if status == 0 {
            let status = GetLastError();
            println!("{:x}",status);
        }
        

        /*
        let status = hide_cursor(0);
        println!("{:x}",status);
        if status == 0 {
            let status = GetLastError();
            println!("{:x}",status);
        }
        */
    }
    std::thread::sleep(time::Duration::from_secs(10))
}