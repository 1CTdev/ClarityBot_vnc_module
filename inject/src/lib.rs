#![allow(unsafe_op_in_unsafe_fn)]

use std::{ffi::c_void, iter, ptr};

use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::{
        Diagnostics::Debug::WriteProcessMemory, LibraryLoader::{GetModuleHandleW, GetProcAddress}, Memory::{MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFree, VirtualFreeEx}, Threading::{
            CreateEventW, CreateRemoteThread, INFINITE, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject
        }
    },
};
use windows_sys::{s, w};

pub unsafe fn create_remote_thread(dll_path: String, pid: u32) -> Result<(), String> {
    let process_handle = OpenProcess(
        PROCESS_CREATE_THREAD | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_QUERY_INFORMATION,
        0,
        pid,
    );
    
    if process_handle == 0 as HANDLE {
        return Err(format!("OpenProcess failed: error code {:x}", GetLastError()));
    }
    
    println!("Opened process: {:?}", process_handle);
    
    let mut dll_path_wide: Vec<u16> = dll_path.encode_utf16().collect();
    dll_path_wide.push(0);
    let dll_path_size = dll_path_wide.len() * 2; // Size in bytes
    
    let remote_memory = VirtualAllocEx(
        process_handle,
        ptr::null(),
        dll_path_size,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );
    
    if remote_memory.is_null() {
        CloseHandle(process_handle);
        return Err(format!("VirtualAllocEx failed: error code {:x}", GetLastError()));
    }
    
    println!("Allocated remote memory: {:?}", remote_memory);
    
    let mut bytes_written: usize = 0;
    let write_result = WriteProcessMemory(
        process_handle,
        remote_memory,
        dll_path_wide.as_ptr() as *const _,
        dll_path_size,
        &mut bytes_written,
    );
    
    if write_result == 0 {
        VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE);
        CloseHandle(process_handle);
        return Err(format!("WriteProcessMemory failed: error code {:x}", GetLastError()));
    }
    
    println!("Wrote {} bytes to remote process", bytes_written);
    
    let kernel32 = GetModuleHandleW(w!("kernel32.dll"));
    if kernel32 == std::ptr::null_mut() {
        VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE);
        CloseHandle(process_handle);
        return Err("Failed to get kernel32.dll handle".to_string());
    }
    
    let load_library_addr = GetProcAddress(kernel32, s!("LoadLibraryW"));
    if load_library_addr.is_none() {
        VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE);
        CloseHandle(process_handle);
        return Err("Failed to get LoadLibraryW address".to_string());
    }
    
    let load_library_fn: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 = 
        std::mem::transmute(load_library_addr.unwrap());
    
    println!("LoadLibraryW address: {:?}", load_library_addr);
    
    let mut thread_id: u32 = 0;
    let remote_thread = CreateRemoteThread(
        process_handle,
        ptr::null(),
        0,
        Some(load_library_fn),
        remote_memory,
        0,
        &mut thread_id,
    );
    
    if remote_thread == 0 as HANDLE {
        VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE);
        CloseHandle(process_handle);
        return Err(format!("CreateRemoteThread failed: error code {:x}", GetLastError()));
    }
    
    println!("Created remote thread: {:?}, thread ID: {}", remote_thread, thread_id);
    
    println!("Waiting for DLL to load...");
    WaitForSingleObject(remote_thread, INFINITE);
    
    println!("Remote thread completed");
    
    CloseHandle(remote_thread);
    VirtualFreeEx(process_handle, remote_memory, 0, MEM_RELEASE);
    CloseHandle(process_handle);
    
    println!("Cleanup complete");
    
    Ok(())
}

pub unsafe fn create_remote_thread_with_event(dll_path: String, pid: u32) -> u32 {
    unsafe {
        let event_name: Vec<u16> = format!("Global\\dldr_{}", pid)
            .encode_utf16()
            .chain(iter::once(0))
            .collect();
        let hinit_event = CreateEventW(std::ptr::null(), 1, 0, event_name.as_ptr());
        if hinit_event.is_null() {
            return GetLastError();
        }

        let hprocess = OpenProcess(PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ, 0, pid);
        if hprocess.is_null() {
            CloseHandle(hinit_event);
            return GetLastError();
        }

        let mut dll_path_wide: Vec<u16> = dll_path.encode_utf16().collect();
        dll_path_wide.push(0);
        let dll_path_size = dll_path_wide.len() * 2;

        let mem = VirtualAllocEx(hprocess, std::ptr::null(), dll_path_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if mem.is_null() {
            CloseHandle(hinit_event);
            CloseHandle(hprocess);
            return GetLastError();
        }

        let mut written:usize=0;
        let status = WriteProcessMemory(hprocess,mem,dll_path_wide.as_ptr() as _,dll_path_size,&mut written);
        if status == 0 {
            VirtualFree(mem, 0, MEM_RELEASE);
            CloseHandle(hinit_event);
            CloseHandle(hprocess);
            return GetLastError();
        }

        let kernel32 = GetModuleHandleW(w!("kernel32.dll")); 

        let loadlibraryw:unsafe extern "system" fn(*mut c_void) -> u32 =  std::mem::transmute(GetProcAddress(kernel32,s!("LoadLibraryW")).unwrap());

        let mut threadid:u32 = 0;

        let hthread = CreateRemoteThread(hprocess, std::ptr::null(), 0, Some(loadlibraryw), mem, 0, &mut threadid);
        if hthread.is_null() {
            VirtualFree(mem, 0, MEM_RELEASE);
            CloseHandle(hinit_event);
            CloseHandle(hprocess);
            return GetLastError();
        }

        println!("created thread (0x{:x}) in process {}",threadid,pid);

        let wait_result = WaitForSingleObject(hthread, 10000);
        match wait_result {
            WAIT_ABANDONED => {
                println!("LoadLibraryW thread wait abandoned");
            },
            WAIT_OBJECT_0 => {
                println!("LoadLibraryW thread completed");
            },
            WAIT_TIMEOUT => {
                println!("LoadLibraryW thread wait timed out");
            },
            WAIT_FAILED => {
                println!("LoadLibraryW thread wait failed");
            },
            _ => {},
        }
        CloseHandle(hthread);

        // Now wait for entry_thread to complete its initialization
        println!("Waiting for hook initialization to complete...");
        let init_wait_result = WaitForSingleObject(hinit_event, 30000); // 30 second timeout
        match init_wait_result {
            WAIT_OBJECT_0 => {
                println!("Hook initialization complete!");
            },
            WAIT_TIMEOUT => {
                println!("Warning: Hook initialization timed out (30s)");
            },
            WAIT_FAILED => {
                println!("Hook initialization wait failed");
            },
            _ => {},
        }
        CloseHandle(hinit_event);
        CloseHandle(hprocess);
    }
    0
}