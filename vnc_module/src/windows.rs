use windows_sys::Win32::{Foundation::{CloseHandle, GENERIC_ALL, HWND, LPARAM}, System::{ProcessStatus::GetModuleFileNameExW, StationsAndDesktops::{EnumDesktopWindows, OpenDesktopW}, Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ}}, UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows, GetWindowDisplayAffinity, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, SW_RESTORE, ShowWindow}};

#[derive(Clone)]
pub struct window_info {
    pub pid:u32,
    pub window_name: String,
    pub module_path: String,
    pub minimized:bool,
    pub affinity:u32,
}

pub unsafe fn running_browser_windows(desktop: *const u16,images:Vec<String>) -> Vec<window_info> {
    let hdesk = OpenDesktopW(desktop,0,0,GENERIC_ALL);

    let browser_windows: Vec<window_info> = desktop_windows(hdesk).iter()
    .filter(|wn| is_valid_browser_window_name(&wn.window_name))
    .filter(|wn| images.iter().any(|x| wn.module_path.eq(x)))
    .cloned()
    .collect();
    browser_windows
}

/// Checks if a window name is a valid browser window (not a system/helper window)
fn is_valid_browser_window_name(window_name: &str) -> bool {
    let trimmed = window_name.trim();
    !trimmed.is_empty()
        && trimmed != "Default IME"
        && trimmed != "MSCTFIME UI"
        && trimmed != "Battery Watcher"
        && trimmed != "WinEventWindow"
}

pub unsafe fn windows() -> Vec<window_info> {
    let mut windows: Vec<window_info> = Vec::new();
    EnumWindows(Some(windowproc),&mut windows as *mut Vec<window_info> as LPARAM);
    windows
}

pub unsafe fn windows_with_affinity_info() -> Vec<window_info> {
    let mut windows: Vec<window_info> = Vec::new();
    EnumWindows(Some(windowproc_with_affinity_info),&mut windows as *mut Vec<window_info> as LPARAM);
    windows
    //https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowdisplayaffinity
}

pub unsafe fn child_windows(parent_hwnd:HWND) -> Vec<window_info> {
    let mut windows: Vec<window_info> = Vec::new();
    EnumChildWindows(parent_hwnd,Some(windowproc),&mut windows as *mut Vec<window_info> as LPARAM);
    windows
}

pub unsafe fn desktop_windows(desktop_handle:HWND) -> Vec<window_info> {
    let mut windows:Vec<window_info> = Vec::new();
    EnumDesktopWindows(desktop_handle,Some(windowproc), &mut windows as *mut Vec<window_info> as LPARAM);
    windows
}

pub unsafe fn first_browser_window() -> HWND {
        let mut hwnd: HWND = std::ptr::null_mut();
        EnumWindows(Some(find_browser_window), &mut hwnd as *mut _ as LPARAM);
        hwnd
}

unsafe extern "system" fn find_browser_window(hwnd:HWND,lparam:LPARAM) -> i32 {
    let browser_hwnd_ptr = lparam as *mut HWND;

    let path_length:u32 = 256*2;
    let mut path_buf: Vec<u16> = Vec::with_capacity(path_length as usize);
    let mut pid:u32 = 0;
    
    GetWindowThreadProcessId(hwnd,&mut pid);
    
    let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_VM_READ,0,pid);
    let path_buf_len = GetModuleFileNameExW(process_handle,std::ptr::null_mut(), path_buf.as_mut_ptr(), path_length);
    path_buf.set_len(path_buf_len as usize);
    CloseHandle(process_handle);

    let module = String::from_utf16(&path_buf).unwrap().to_lowercase();
    
    let window_length = GetWindowTextLengthW(hwnd)+2;

    let mut title_buf: Vec<u16> = Vec::with_capacity(window_length as usize);

    let title_buf_len = GetWindowTextW(hwnd,title_buf.as_mut_ptr(),window_length);
    title_buf.set_len(title_buf_len as usize);
    let title = String::from_utf16(&title_buf).unwrap();

    if !module.contains("msedge.exe") && !module.contains("firefox.exe") && !module.contains("chrome.exe") {
        return 1;
    }
    if !is_valid_browser_window_name(&title) {
        return 1;
    }

    if IsIconic(hwnd) != 0 {
        ShowWindow(hwnd, SW_RESTORE);
    }

    #[cfg(debug_assertions)]
    {
        println!("browser window: {}",title.trim());
    }

    (*browser_hwnd_ptr) = hwnd;
    0
}

//https://stackoverflow.com/questions/277085/how-do-i-getmodulefilename-if-i-only-have-a-window-handle-hwnd

/*
BOOL CALLBACK EnumWindowsProc(
  _In_ HWND   hwnd,
  _In_ LPARAM lParam
);
*/
//https://learn.microsoft.com/en-us/previous-versions/windows/desktop/legacy/ms633498(v=vs.85)
unsafe extern "system" fn windowproc(hwnd:HWND,lparam:LPARAM) -> i32 {
    let windows_vec = lparam as *mut Vec<window_info>;

    let path_length:u32 = 256*2;
    let mut path_buf: Vec<u16> = Vec::with_capacity(path_length as usize);
    let mut pid:u32 = 0;
    
    GetWindowThreadProcessId(hwnd,&mut pid);
    
    let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_VM_READ,0,pid);
    let path_buf_len = GetModuleFileNameExW(process_handle,std::ptr::null_mut(), path_buf.as_mut_ptr(), path_length);
    path_buf.set_len(path_buf_len as usize);
    CloseHandle(process_handle);

    let window_length = GetWindowTextLengthW(hwnd)+2;

    let mut title_buf: Vec<u16> = Vec::with_capacity(window_length as usize);

    let title_buf_len = GetWindowTextW(hwnd,title_buf.as_mut_ptr(),window_length);
    title_buf.set_len(title_buf_len as usize);


    let minimized = IsIconic(hwnd)!=0;
    let title = String::from_utf16(&title_buf).unwrap();
    let module = String::from_utf16(&path_buf).unwrap();

    (*windows_vec).push(window_info{pid:pid,window_name:title,module_path:module,minimized:minimized,affinity:0});
    1
}


unsafe extern "system" fn windowproc_with_affinity_info(hwnd:HWND,lparam:LPARAM) -> i32 {
    let windows_vec = lparam as *mut Vec<window_info>;

    let path_length:u32 = 256*2;
    let mut path_buf: Vec<u16> = Vec::with_capacity(path_length as usize);
    let mut pid:u32 = 0;
    
    GetWindowThreadProcessId(hwnd,&mut pid);
    
    let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_VM_READ,0,pid);
    let path_buf_len = GetModuleFileNameExW(process_handle,std::ptr::null_mut(), path_buf.as_mut_ptr(), path_length);
    path_buf.set_len(path_buf_len as usize);
    CloseHandle(process_handle);

    let window_length = GetWindowTextLengthW(hwnd)+2;

    let mut title_buf: Vec<u16> = Vec::with_capacity(window_length as usize);

    let title_buf_len = GetWindowTextW(hwnd,title_buf.as_mut_ptr(),window_length);
    title_buf.set_len(title_buf_len as usize);


    let minimized = IsIconic(hwnd)!=0;
    let title = String::from_utf16(&title_buf).unwrap();
    let module = String::from_utf16(&path_buf).unwrap();

    let mut affinity:u32 = 0;
    GetWindowDisplayAffinity(hwnd,&mut affinity);

    (*windows_vec).push(window_info{pid:pid,window_name:title,module_path:module,minimized:minimized,affinity:affinity});
    1
}