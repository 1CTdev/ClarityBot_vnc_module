use std::{collections::HashMap, mem::size_of, sync::{atomic::{AtomicBool, AtomicIsize, Ordering}, Arc, OnceLock, RwLock}, thread::{self, JoinHandle}};

use windows_sys::{
    Wdk::Foundation::NtClose, 
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM}, 
        Graphics::Gdi::{
            CreateSolidBrush, DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, 
            GetDC, GetMonitorInfoW, GetStockObject, HDC, HMONITOR, MONITORINFOEXW, 
            ReleaseDC, BLACK_BRUSH, PAINTSTRUCT, BeginPaint, EndPaint, FillRect,
        }, 
        System::Registry::RegCloseKey, 
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, 
            GetDesktopWindow, GetMessageW, PostMessageW, RegisterClassExW, 
            SetWindowDisplayAffinity, ShowWindow, TranslateMessage, 
            CS_HREDRAW, CS_VREDRAW, MSG, SW_HIDE, SW_SHOWNA, WM_DESTROY, WM_PAINT, 
            WM_QUIT, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, 
            WS_EX_TOPMOST, WS_POPUP, WS_EX_TRANSPARENT, WS_EX_LAYERED,
            SetLayeredWindowAttributes, LWA_ALPHA,
        },
    }, 
    core::BOOL
};

use crate::types::MONITOR_INFO;

use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, KEY_READ, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW};

const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

static BLACKOUT_RUNNING: AtomicBool = AtomicBool::new(false);
static BLACKOUT_HWND: AtomicIsize = AtomicIsize::new(0);
static BLACKOUT_THREAD: OnceLock<RwLock<Option<JoinHandle<()>>>> = OnceLock::new();

static AVAILABLE_MONITORS: OnceLock<RwLock<HashMap<String, MONITOR_INFO>>> = OnceLock::new();


pub fn available_monitors_global() -> &'static RwLock<HashMap<String, MONITOR_INFO>> {
    AVAILABLE_MONITORS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn available_monitors_insert_kv(k: impl Into<String>, v: impl Into<MONITOR_INFO>) {
    let lock = available_monitors_global();
    let mut map = lock.write().unwrap();
    map.insert(k.into(), v.into());
}

pub fn available_monitors_get_k(k: impl Into<String>) -> Option<MONITOR_INFO> {
    let lock = available_monitors_global();
    let map = lock.read().unwrap();
    map.get(&k.into()).cloned()
}

pub fn available_monitors_delete_k(k: impl Into<String>) {
    let lock = available_monitors_global();
    let mut map = lock.write().unwrap();
    map.remove(&k.into());
}

/// Window procedure for blackout overlay - handles messages to stay responsive
/// Input passes through via WS_EX_TRANSPARENT
unsafe extern "system" fn blackout_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_null() {
                let brush = GetStockObject(BLACK_BRUSH as i32);
                FillRect(hdc, &ps.rcPaint, brush as _);
                EndPaint(hwnd, &ps);
            }
            0
        }
        WM_DESTROY => {
            PostMessageW(std::ptr::null_mut(), WM_QUIT, 0, 0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn get_virtual_screen_bounds() -> (i32, i32, i32, i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, 
        SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN
    };
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let cx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let cy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        (x, y, cx, cy)
    }
}

fn create_blackout_window() -> HWND {
    unsafe {
        let class_name: Vec<u16> = "Blackout\0".encode_utf16().collect();
        
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(blackout_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: std::ptr::null_mut(),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(), // No cursor on blackout
            hbrBackground: GetStockObject(BLACK_BRUSH as i32) as _,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        
        RegisterClassExW(&wc);
        
        let (x, y, width, height) = get_virtual_screen_bounds();

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            x,
            y,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        
        if hwnd.is_null() {
            return std::ptr::null_mut();
        }

        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        
        ShowWindow(hwnd, SW_SHOWNA);
        
        hwnd
    }
}

fn run_blackout_message_loop() {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        
        while BLACKOUT_RUNNING.load(Ordering::SeqCst) {
            let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            
            if ret == 0 || ret == -1 {
                break;
            }
            
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Enables or disables the blackout overlay
/// block: 1 = enable blackout, 0 = disable blackout
/// Returns: 1 on success, 0 on failure
pub fn disable_monitors(block: i32) -> i32 {
    let thread_lock = BLACKOUT_THREAD.get_or_init(|| RwLock::new(None));
    
    if block == 1 {
        if BLACKOUT_RUNNING.load(Ordering::SeqCst) {
            return 1;
        }
        
        BLACKOUT_RUNNING.store(true, Ordering::SeqCst);
        
        let handle = thread::spawn(|| {
            let hwnd = create_blackout_window();
            if hwnd.is_null() {
                BLACKOUT_RUNNING.store(false, Ordering::SeqCst);
                return;
            }
            
            BLACKOUT_HWND.store(hwnd as isize, Ordering::SeqCst);
            
            run_blackout_message_loop();
            
            unsafe {
                let hwnd = BLACKOUT_HWND.swap(0, Ordering::SeqCst);
                if hwnd != 0 {
                    DestroyWindow(hwnd as HWND);
                }
            }
        });
        
        if let Ok(mut guard) = thread_lock.write() {
            *guard = Some(handle);
        }
        
        1
    } else {
        if !BLACKOUT_RUNNING.load(Ordering::SeqCst) {
            return 1;
        }
        
        BLACKOUT_RUNNING.store(false, Ordering::SeqCst);
        
        unsafe {
            let hwnd = BLACKOUT_HWND.load(Ordering::SeqCst);
            if hwnd != 0 {
                PostMessageW(hwnd as HWND, WM_DESTROY, 0, 0);
            }
        }
        
        if let Ok(mut guard) = thread_lock.write() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
        
        1
    }
}


pub fn parse_edid_manufacturer(edid: &[u8]) -> String {
    if edid.len() < 10 {
        return String::new();
    }
    let byte1 = edid[8];
    let byte2 = edid[9];
    
    let char1 = ((byte1 >> 2) & 0x1F) + b'A' - 1;
    let char2 = (((byte1 & 0x03) << 3) | ((byte2 >> 5) & 0x07)) + b'A' - 1;
    let char3 = (byte2 & 0x1F) + b'A' - 1;
    
    String::from_utf8(vec![char1, char2, char3]).unwrap_or_default()
}

pub fn parse_edid_monitor_name(edid: &[u8]) -> String {
    if edid.len() < 128 {
        return String::new();
    }
    
    for i in 0..4 {
        let offset = 54 + (i * 18);
        if offset + 18 > edid.len() {
            break;
        }
        
        if edid[offset] == 0x00 && edid[offset + 1] == 0x00 
            && edid[offset + 2] == 0x00 && edid[offset + 3] == 0xFC {
            let name_bytes = &edid[offset + 5..offset + 18];
            let name: String = name_bytes
                .iter()
                .take_while(|&&b| b != 0x0A && b != 0x00) // Name ends at newline or null
                .map(|&b| b as char)
                .collect();
            return name.trim().to_string();
        }
    }
    
    String::new()
}

unsafe fn get_edid_from_registry(device_id: &str) -> Option<Vec<u8>> {
    if device_id.is_empty() {
        return None;
    }
    
    let parts: Vec<&str> = device_id.split('\\').collect();
    if parts.len() < 2 {
        return None;
    }
    let hardware_id = parts[1];
    
    let display_key_path: Vec<u16> = format!(r#"SYSTEM\CurrentControlSet\Enum\DISPLAY\{}"#, hardware_id)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    
    let mut display_key: windows_sys::Win32::System::Registry::HKEY = std::ptr::null_mut();
    if RegOpenKeyExW(
        HKEY_LOCAL_MACHINE,
        display_key_path.as_ptr(),
        0,
        KEY_READ,
        &mut display_key,
    ) != 0 {
        return None;
    }

    let mut index: u32 = 0;
    let mut edid_data: Option<Vec<u8>> = None;
    
    loop {
        let mut subkey_name: [u16; 256] = [0; 256];
        let mut subkey_name_len: u32 = 256;
        
        let result = RegEnumKeyExW(
            display_key,
            index,
            subkey_name.as_mut_ptr(),
            &mut subkey_name_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        
        if result != 0 {
            break;
        }

        let instance_name = String::from_utf16_lossy(&subkey_name[..subkey_name_len as usize]);
        let params_path: Vec<u16> = format!(r#"{}\Device Parameters"#, instance_name)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        
        let mut params_key: windows_sys::Win32::System::Registry::HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            display_key,
            params_path.as_ptr(),
            0,
            KEY_READ,
            &mut params_key,
        ) == 0 {
            let edid_name: Vec<u16> = "EDID\0".encode_utf16().collect();
            let mut data_type: u32 = 0;
            let mut data_size: u32 = 256;
            let mut data: Vec<u8> = vec![0; 256];
            
            if RegQueryValueExW(
                params_key,
                edid_name.as_ptr(),
                std::ptr::null_mut(),
                &mut data_type,
                data.as_mut_ptr(),
                &mut data_size,
            ) == 0 && data_size >= 128 {
                data.truncate(data_size as usize);
                edid_data = Some(data);
            }
            
            RegCloseKey(params_key);
            
            if edid_data.is_some() {
                break;
            }
        }
        
        index += 1;
    }
    
    RegCloseKey(display_key);
    edid_data
}

unsafe fn get_monitor_info_from_edid(device_id: &str) -> (String, String) {
    if let Some(edid) = get_edid_from_registry(device_id) {
        let manufacturer = parse_edid_manufacturer(&edid);
        let model_name = parse_edid_monitor_name(&edid);
        (model_name, manufacturer)
    } else {
        (String::new(), String::new())
    }
}

unsafe extern "system" fn EnumMonitorProc(hMonitor: HMONITOR, hdcMonitor: HDC, lprcMonitor: *mut RECT, dwData: LPARAM) -> BOOL {
    let monitors_vec = dwData as *mut Vec<MONITOR_INFO>;
    if monitors_vec.is_null() {
        return 0;
    }
    
    let mut mi: MONITORINFOEXW = std::mem::zeroed();
    mi.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    
    if GetMonitorInfoW(hMonitor, &mut mi.monitorInfo as *mut _ as *mut _) == 0 {
        return 1;
    }
    
    let name = String::from_utf16_lossy(&mi.szDevice)
        .trim_end_matches('\0')
        .to_string();
    
    let is_primary = (mi.monitorInfo.dwFlags & 1) != 0;

    let mut display_device: DISPLAY_DEVICEW = std::mem::zeroed();
    display_device.cb = size_of::<DISPLAY_DEVICEW>() as u32;
    
    let has_monitor = EnumDisplayDevicesW(mi.szDevice.as_ptr(), 0, &mut display_device, 0) != 0;
    
    let device_name = String::from_utf16_lossy(&display_device.DeviceName)
        .trim_end_matches('\0')
        .to_string();
    
    let device_string = String::from_utf16_lossy(&display_device.DeviceString)
        .trim_end_matches('\0')
        .to_string();

    let device_id = String::from_utf16_lossy(&display_device.DeviceID)
        .trim_end_matches('\0')
        .to_string();
    
    let state_flags = display_device.StateFlags;

    const DISPLAY_DEVICE_MIRRORING_DRIVER: u32 = 0x00000008;
    let is_virtual = !has_monitor 
        || (state_flags & DISPLAY_DEVICE_MIRRORING_DRIVER) != 0
        || device_id.is_empty();
    
    let (model_name, manufacturer) = get_monitor_info_from_edid(&device_id);
    
    let monitor_info = MONITOR_INFO {
        name: name,
        device_name: device_name,
        device_string: device_string,
        device_id: device_id,
        model_name: model_name,
        manufacturer: manufacturer,
        state_flags: state_flags,
        is_virtual: is_virtual,
        is_primary: is_primary,
        monitor_rect: mi.monitorInfo.rcMonitor,
        work_rect: mi.monitorInfo.rcWork,
    };
    
    (*monitors_vec).push(monitor_info);
    
    1
}

pub fn get_monitors() -> Result<Vec<MONITOR_INFO>, String> {
    let mut monitors: Vec<MONITOR_INFO> = Vec::new();
    
    unsafe {
        let hwnd = GetDesktopWindow();
        let hdc_monitor = GetDC(hwnd);
        
        let result = EnumDisplayMonitors(
            hdc_monitor,
            std::ptr::null(),
            Some(EnumMonitorProc),
            &mut monitors as *mut Vec<MONITOR_INFO> as LPARAM,
        );

        ReleaseDC(hwnd, hdc_monitor);
        NtClose(hwnd);
        
        if result == 0 {
            return Err("Failed to enumerate monitors".to_string());
        }
    }

    for (i, monitor) in monitors.iter().enumerate() {
        available_monitors_insert_kv(monitor.name.clone().replace(r"\\.\", ""), monitor.clone());
    }
    
    Ok(monitors)
}