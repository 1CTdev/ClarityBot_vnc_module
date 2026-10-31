use std::{slice, sync::{OnceLock, RwLock}};

use windows_sys::{Win32::{Foundation::{CloseHandle, ERROR_SUCCESS, GENERIC_ALL, GetLastError, HANDLE}, Security::{GetTokenInformation, SID_AND_ATTRIBUTES, TokenIntegrityLevel}, System::{Memory::{LPTR, LocalAlloc}, Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, REG_DWORD, RegCreateKeyW, RegOpenKeyExW, RegSetValueExW}, StationsAndDesktops::{CloseDesktop, CreateDesktopW, GetThreadDesktop, GetUserObjectInformationW, OpenInputDesktop, SetThreadDesktop, UOI_IO, UOI_NAME}, Threading::{CREATE_UNICODE_ENVIRONMENT, CreateProcessW, CreateProcessWithTokenW, DETACHED_PROCESS, GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenProcessToken, PROCESS_INFORMATION, STARTUPINFOW}}}, w};

#[derive(Clone, Default)]
pub struct CachedDesktopState {
    desktop_name: String,
    is_hidden_desktop: bool,
}

static CACHED_DESKTOP_STATE: OnceLock<RwLock<CachedDesktopState>> = OnceLock::new();

pub fn cached_desktop_global() -> &'static RwLock<CachedDesktopState> {
    CACHED_DESKTOP_STATE.get_or_init(|| RwLock::new(CachedDesktopState::default()))
}

pub fn update_cached_desktop_state(desktop_name: String, is_hidden: bool) {
    let lock = cached_desktop_global();
    let mut state = lock.write().unwrap();
    state.desktop_name = desktop_name;
    state.is_hidden_desktop = is_hidden;
}

pub fn is_hidden_desktop_cached() -> bool {
    let lock = cached_desktop_global();
    lock.read().unwrap().is_hidden_desktop
}

/*
UOI_IO
6
TRUE if the hObj parameter is a handle to the desktop object that is receiving input from the user. FALSE otherwise.
Windows Server 2003 and Windows XP/2000:  This value is not supported.

UOI_NAME
2
The name of the object, as a string.
*/
    //https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getthreaddesktop
    //GetThreadDesktop
    //https://learn.microsoft.com/en-us/windows/desktop/api/processthreadsapi/nf-processthreadsapi-getcurrentthreadid
    //GetCurrentThreadId
    //https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getuserobjectinformationw
    //GetUserObjectInformationW

#[unsafe(no_mangle)]
pub unsafe fn current_desktop() -> Result<(String,bool),u32> {
    let hwnd = GetThreadDesktop(GetCurrentThreadId());
    let mut buf:Vec<u8> = vec![0;256];
    let mut bytes_written:u32 = 0;
    let status = GetUserObjectInformationW(hwnd, UOI_NAME, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut bytes_written);
    if status == 0 {
        return Err(GetLastError());
    }
    let desktop_name = String::from_utf16(slice::from_raw_parts(buf.as_ptr() as *const u16, (bytes_written / 2) as usize)).unwrap();

    let mut buf:Vec<u8> = vec![0;4];
    let mut bytes_written:u32 = 0;
    let status = GetUserObjectInformationW(hwnd, UOI_IO, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut bytes_written);
    if status == 0 {
        return Err(GetLastError());
    }
    let is_input_desktop = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) != 0;
    Ok((desktop_name, is_input_desktop))
}


pub unsafe fn task_bar_glom_level() -> u32 {
    let value:u32 = 2;
    let value = u32::to_le_bytes(value);
    let mut hkey:HANDLE = Default::default();
    let result = RegOpenKeyExW(HKEY_CURRENT_USER, w!(r#"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced\"#), 0, KEY_ALL_ACCESS,&mut hkey);
    if result != ERROR_SUCCESS {
        return result;
    }
    RegCreateKeyW(hkey, w!("TaskbarGlomLevel"), std::ptr::null_mut());
    RegSetValueExW(hkey,w!("TaskbarGlomLevel"),0,REG_DWORD,&value as *const _,4)
}

pub unsafe fn start_explorer(desktop:*const u16) -> Result<PROCESS_INFORMATION,u32> {
    let mut my_process_token:HANDLE = Default::default();
    OpenProcessToken(GetCurrentProcess(), 0x2000000, &mut my_process_token);

    let mut startup_info: STARTUPINFOW = Default::default();
    startup_info.cb = size_of::<STARTUPINFOW>() as u32;
    startup_info.lpDesktop = desktop as *mut u16;
    let mut process_info: PROCESS_INFORMATION = Default::default();
    
    if get_integrity_level(my_process_token) > SECURITY_MANDATORY_MEDIUM_RID {
        let input_desktop_handle = OpenInputDesktop(0,0,GENERIC_ALL);
        let windows = crate::windows::desktop_windows(input_desktop_handle);
        for window in windows  {
            if window.window_name.contains("Program Manager") {
                let explorer_handle = OpenProcess(window.pid,1,GENERIC_ALL);
                let mut explorer_token:HANDLE = Default::default();
                OpenProcessToken(explorer_handle, GENERIC_ALL, &mut explorer_token);

                CreateProcessWithTokenW(
                    explorer_token,
                    0,
                    w!(r#"C:\Windows\explorer.exe"#),
                    std::ptr::null_mut(),
                    CREATE_UNICODE_ENVIRONMENT|DETACHED_PROCESS,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut startup_info,
                    &mut process_info,
                );
                CloseHandle(explorer_token);
                CloseHandle(explorer_handle);
            }
        }
        CloseDesktop(input_desktop_handle);
    } else {
        CreateProcessW(
            w!(r#"C:\Windows\explorer.exe"#),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            CREATE_UNICODE_ENVIRONMENT|DETACHED_PROCESS,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut startup_info,
            &mut process_info,
        );
    }
    CloseHandle(my_process_token);
    if process_info.hProcess == std::ptr::null_mut() {
        return Err(GetLastError());
    }
    Ok(process_info)
}

const SECURITY_MANDATORY_MEDIUM_RID: u32 = 0x00003000;
unsafe fn get_integrity_level(token:HANDLE) -> u32 {
        let mut bytes_required: u32 = 0;
        GetTokenInformation(token, TokenIntegrityLevel, std::ptr::null_mut(), 0, &mut bytes_required);

        let buffer = LocalAlloc(LPTR, bytes_required as usize);

        GetTokenInformation(token, TokenIntegrityLevel, buffer as *mut _, bytes_required, &mut bytes_required);

        let token_level = buffer.cast::<SID_AND_ATTRIBUTES>();
        (*token_level).Attributes
} 

//EDGE_USER_DATA_DIR, C:\Users\user\AppData\Local\Microsoft\Edge\User Data


pub unsafe fn disable_secure_desktop() -> u32 {
    let value:u32 = 0;
    let value = u32::to_le_bytes(value);
    let mut hkey:HANDLE = Default::default();
    let result = RegOpenKeyExW(HKEY_LOCAL_MACHINE, w!(r#"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System\"#), 0, KEY_ALL_ACCESS,&mut hkey);
    if result != ERROR_SUCCESS {
        return result;
    }
    RegCreateKeyW(hkey, w!("PromptOnSecureDesktop"), std::ptr::null_mut());
    RegSetValueExW(hkey,w!("PromptOnSecureDesktop"),0,REG_DWORD,&value as *const _,4)
}