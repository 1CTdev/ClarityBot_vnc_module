

//https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values#code_values_on_windows <- this page shows scan codes not virtual key codes

//https://learn.microsoft.com/en-us/uwp/api/windows.system.virtualkey?view=winrt-26100

use std::{collections::HashMap, sync::{OnceLock, RwLock}};

use windows_sys::Win32::{Foundation::{HWND, LPARAM}, UI::{Input::KeyboardAndMouse::{BlockInput, GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SendInput, ToUnicodeEx}, WindowsAndMessaging::{EnumWindows, GetSystemMetrics, GetWindowThreadProcessId, PostMessageW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2}}};

use crate::{get_capture_dimensions, types::{self, PROTOCOL_KEY_DIRECTION}};

const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_SHIFT: usize = 0x0004;
const MK_CONTROL: usize = 0x0008;
const MK_MBUTTON: usize = 0x0010;
const MK_XBUTTON1: usize = 0x0020;
const MK_XBUTTON2: usize = 0x0040;

const VK_SHIFT: u8 = 0x10;
const VK_CONTROL: u8 = 0x11;
const VK_MENU: u8 = 0x12; // Alt key
const VK_LSHIFT: u8 = 0xA0;
const VK_RSHIFT: u8 = 0xA1;
const VK_LCONTROL: u8 = 0xA2;
const VK_RCONTROL: u8 = 0xA3;
const VK_LMENU: u8 = 0xA4;
const VK_RMENU: u8 = 0xA5;
const VK_CAPITAL: u8 = 0x14; // Caps Lock
const VK_NUMLOCK: u8 = 0x90;
const VK_SCROLL: u8 = 0x91;

static CAPS_LOCK_STATE: OnceLock<RwLock<bool>> = OnceLock::new();
static NUM_LOCK_STATE: OnceLock<RwLock<bool>> = OnceLock::new();

fn caps_lock_global() -> &'static RwLock<bool> {
    CAPS_LOCK_STATE.get_or_init(|| RwLock::new(false))
}

fn num_lock_global() -> &'static RwLock<bool> {
    NUM_LOCK_STATE.get_or_init(|| RwLock::new(true)) // Num Lock typically starts ON
}

fn toggle_caps_lock() {
    let lock = caps_lock_global();
    let mut state = lock.write().unwrap();
    *state = !*state;
}

fn toggle_num_lock() {
    let lock = num_lock_global();
    let mut state = lock.write().unwrap();
    *state = !*state;
}

fn is_caps_lock_on() -> bool {
    let lock = caps_lock_global();
    *lock.read().unwrap()
}

fn is_num_lock_on() -> bool {
    let lock = num_lock_global();
    *lock.read().unwrap()
}

/// Creates an lParam value from x,y coordinates (MAKELPARAM equivalent)
/// lParam format: low-order word = x, high-order word = y
fn make_lparam(x: i32, y: i32) -> isize {
    let x_word = (x & 0xFFFF) as u16;
    let y_word = (y & 0xFFFF) as u16;
    ((y_word as u32) << 16 | x_word as u32) as isize
}

static KEY_STATE: OnceLock<RwLock<HashMap<u8, PROTOCOL_KEY_DIRECTION>>> = OnceLock::new();

fn key_state_global() -> &'static RwLock<HashMap<u8, PROTOCOL_KEY_DIRECTION>> {
    KEY_STATE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn key_state_insert_kv(k: impl Into<u8>, v: impl Into<PROTOCOL_KEY_DIRECTION>) {
    let lock = key_state_global();
    let mut map = lock.write().unwrap();
    map.insert(k.into(), v.into());
}

fn key_state_get_k(k: impl Into<u8>) -> Option<PROTOCOL_KEY_DIRECTION> {
    let lock = key_state_global();
    let map = lock.read().unwrap();
    map.get(&k.into()).cloned()
}

fn get_modifier_state() -> usize {
    let mut modifiers: usize = 0;
    
    if key_state_get_k(VK_SHIFT) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
        || key_state_get_k(VK_LSHIFT) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
        || key_state_get_k(VK_RSHIFT) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
    {
        modifiers |= MK_SHIFT;
    }
    
    if key_state_get_k(VK_CONTROL) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
        || key_state_get_k(VK_LCONTROL) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
        || key_state_get_k(VK_RCONTROL) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
    {
        modifiers |= MK_CONTROL;
    }
    
    modifiers
}

pub unsafe fn mouse_button_wrapper(ishiddendesktop:bool,x:i32,y:i32,virtual_key_code:u8,direction:PROTOCOL_KEY_DIRECTION) -> i32 {
    if ishiddendesktop {
        let mut hwnd = crate::windows::first_browser_window();
        if hwnd.is_null() {
            return 0;
        }
        
        let modifiers = get_modifier_state();
        
        let (msg, mut wparam): (u32, usize) = match virtual_key_code {
            1 => { // Left button
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    (WM_LBUTTONDOWN, MK_LBUTTON)
                } else {
                    (WM_LBUTTONUP, 0)
                }
            },
            2 => { // Right button
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    (WM_RBUTTONDOWN, MK_RBUTTON)
                } else {
                    (WM_RBUTTONUP, 0)
                }
            },
            4 => { // Middle button
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    (WM_MBUTTONDOWN, MK_MBUTTON)
                } else {
                    (WM_MBUTTONUP, 0)
                }
            },
            5 => { // X Button 1 (back)
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    (WM_XBUTTONDOWN, MK_XBUTTON1 | ((XBUTTON1 as usize) << 16))
                } else {
                    (WM_XBUTTONUP, (XBUTTON1 as usize) << 16)
                }
            },
            6 => { // X Button 2 (forward)
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    (WM_XBUTTONDOWN, MK_XBUTTON2 | ((XBUTTON2 as usize) << 16))
                } else {
                    (WM_XBUTTONUP, (XBUTTON2 as usize) << 16)
                }
            },
            _ => return 0, // Unknown button
        };
        
        wparam |= modifiers;
        
        return PostMessageW(hwnd, msg, wparam, make_lparam(x, y));
    } else {
        // Convert monitor-relative coordinates to virtual screen coordinates
        let offset = crate::get_capture_offset();
        let virtual_x = x + offset.x;
        let virtual_y = y + offset.y;

        // Get virtual screen bounds (covers all monitors)
        let virtual_screen_x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let virtual_screen_y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let virtual_screen_width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let virtual_screen_height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        // Normalize coordinates relative to the entire virtual screen
        // MOUSEEVENTF_ABSOLUTE with MOUSEEVENTF_VIRTUALDESK uses 0-65535 range across all monitors
        let normalized_x = ((virtual_x - virtual_screen_x) * 65535) / virtual_screen_width;
        let normalized_y = ((virtual_y - virtual_screen_y) * 65535) / virtual_screen_height;

        let flags:u32;
        match virtual_key_code {
            1 => {//left
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    flags=MOUSEEVENTF_LEFTDOWN
                } else {
                    flags=MOUSEEVENTF_LEFTUP
                }
                },
            2 => {//right
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    flags=MOUSEEVENTF_RIGHTDOWN
                } else {
                    flags=MOUSEEVENTF_RIGHTUP
                }
            },
            4 => {//middle
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    flags=MOUSEEVENTF_MIDDLEDOWN
                } else {
                    flags=MOUSEEVENTF_MIDDLEUP
                }
            },
            _ => {
                if direction == PROTOCOL_KEY_DIRECTION::DOWN {
                    flags=MOUSEEVENTF_XDOWN
                } else {
                    flags=MOUSEEVENTF_XUP
                }
            },
        }
        //https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
        let mouse_input_button:MOUSEINPUT = MOUSEINPUT {
            dx: normalized_x, 
            dy: normalized_y,
            mouseData: 0,
            dwFlags: flags|MOUSEEVENTF_ABSOLUTE|MOUSEEVENTF_VIRTUALDESK|MOUSEEVENTF_MOVE, 
            time: 0,
            dwExtraInfo: 0 };
        let mouse_input_move:MOUSEINPUT = MOUSEINPUT {
            dx: normalized_x,
            dy: normalized_y,
            mouseData: 0,
            dwFlags: MOUSEEVENTF_ABSOLUTE|MOUSEEVENTF_VIRTUALDESK|MOUSEEVENTF_MOVE, 
            time: 0,
            dwExtraInfo: 0 };

        let mut inputs: [INPUT; 2] = [std::mem::zeroed(); 2];
        inputs[0] = INPUT {
            r#type:INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: mouse_input_move,
            },
        };
        inputs[1] = INPUT {
            r#type:INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: mouse_input_button,
            },
        };
        return SendInput(2,&inputs as *const _,size_of::<INPUT>() as i32) as i32;
    }
}


//https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getfocus
//https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow
pub unsafe fn keyboard_button_wrapper(ishiddendesktop:bool,virtual_key_code:u8,scan_code:u32,direction:PROTOCOL_KEY_DIRECTION) -> i32 {
    if ishiddendesktop {
        //let mut hwnd: HWND = std::ptr::null_mut();
        //EnumWindows(Some(find_browser_window), &mut hwnd as *mut _ as LPARAM);
        let mut hwnd = crate::windows::first_browser_window();
        if hwnd.is_null() {
            return 0;
        }
        
        if direction == PROTOCOL_KEY_DIRECTION::DOWN {
            if virtual_key_code == VK_CAPITAL {
                toggle_caps_lock();
            } else if virtual_key_code == VK_NUMLOCK {
                toggle_num_lock();
            }
        }
        
        let alt_held = key_state_get_k(VK_MENU) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
            || key_state_get_k(VK_LMENU) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
            || key_state_get_k(VK_RMENU) == Some(PROTOCOL_KEY_DIRECTION::DOWN);
        
        let ctrl_held = key_state_get_k(VK_CONTROL) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
            || key_state_get_k(VK_LCONTROL) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
            || key_state_get_k(VK_RCONTROL) == Some(PROTOCOL_KEY_DIRECTION::DOWN);
        
        let shift_held = key_state_get_k(VK_SHIFT) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
            || key_state_get_k(VK_LSHIFT) == Some(PROTOCOL_KEY_DIRECTION::DOWN)
            || key_state_get_k(VK_RSHIFT) == Some(PROTOCOL_KEY_DIRECTION::DOWN);
        
        // Build lParam for WM_KEYDOWN/WM_KEYUP/WM_CHAR:
        // Bits 0-15: Repeat count (1)
        // Bits 16-23: Scan code
        // Bit 24: Extended key flag (1 if extended)
        // Bits 25-28: Reserved (0)
        // Bit 29: Context code (1 for SYSKEY with Alt, 0 otherwise)
        // Bit 30: Previous key state (1 if was down, 0 if was up)
        // Bit 31: Transition state (0 for DOWN, 1 for UP)
        
        let repeat_count: u32 = 1;
        let scan_code_bits: u32 = (scan_code & 0xFF) << 16;
        let extended_bit: u32 = if scan_code & 0xE000 != 0 { 1 << 24 } else { 0 };
        let context_bit: u32 = if alt_held { 1 << 29 } else { 0 };
        
        let previous_key_state = key_state_get_k(virtual_key_code).unwrap_or_default();
        let prev_state_bit: u32 = if previous_key_state == PROTOCOL_KEY_DIRECTION::DOWN { 1 << 30 } else { 0 };
        
        // Build keyboard state array for ToUnicodeEx
        // This is a 256-byte array where:
        // - High bit (0x80) indicates key is currently down
        // - Low bit (0x01) indicates toggle state for Caps Lock, Num Lock, Scroll Lock
        let mut keyboard_state: [u8; 256] = [0; 256];
        
        if shift_held {
            keyboard_state[VK_SHIFT as usize] = 0x80;
        }
        if ctrl_held {
            keyboard_state[VK_CONTROL as usize] = 0x80;
        }
        if alt_held {
            keyboard_state[VK_MENU as usize] = 0x80;
        }
        
        if is_caps_lock_on() {
            keyboard_state[VK_CAPITAL as usize] = 0x01;
        }
        if is_num_lock_on() {
            keyboard_state[VK_NUMLOCK as usize] = 0x01;
        }
        
        let thread_id = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let hkl = GetKeyboardLayout(thread_id);
        
        let mut char_buffer: [u16; 8] = [0; 8];
        let result = ToUnicodeEx(
            virtual_key_code as u32,
            scan_code,
            keyboard_state.as_ptr(),
            char_buffer.as_mut_ptr(),
            char_buffer.len() as i32,
            0, // flags: 0 = no menu active
            hkl,
        );
        
        // result > 0: number of characters written
        // result == 0: no translation for this key
        // result < 0: dead key (accent, etc.)
        //
        // IMPORTANT: ToUnicodeEx returns 1 for control characters like Backspace (0x08),
        // Enter (0x0D), Tab (0x09), Escape (0x1B), etc. These should NOT be sent as WM_CHAR
        // because Firefox and other apps expect WM_KEYDOWN for these keys to work properly.
        // Only characters >= 0x20 (space) are truly "printable" and should use WM_CHAR.
        let is_printable_char = result > 0 && char_buffer[0] >= 0x20;
        let is_dead_key = result < 0;
        
        let is_shortcut = is_printable_char && (ctrl_held || alt_held);
        
        if direction == PROTOCOL_KEY_DIRECTION::UP {
            let msg = if alt_held { WM_SYSKEYUP } else { WM_KEYUP };
            let transition_bit: u32 = 1 << 31;
            let lparam: u32 = repeat_count | scan_code_bits | extended_bit | context_bit | prev_state_bit | transition_bit;
            PostMessageW(hwnd, msg, virtual_key_code as usize, lparam as isize);
        } else {
            let transition_bit: u32 = 0;
            let lparam: u32 = repeat_count | scan_code_bits | extended_bit | context_bit | prev_state_bit | transition_bit;
            
            if is_printable_char && !is_shortcut {
                let char_msg = if alt_held { WM_SYSCHAR } else { WM_CHAR };
                for i in 0..(result as usize) {
                    let char_code = char_buffer[i];
                    PostMessageW(hwnd, char_msg, char_code as usize, lparam as isize);
                }
            } else if is_dead_key {
                let msg = if alt_held { WM_SYSKEYDOWN } else { WM_KEYDOWN };
                PostMessageW(hwnd, msg, virtual_key_code as usize, lparam as isize);
            } else {
                let msg = if alt_held { WM_SYSKEYDOWN } else { WM_KEYDOWN };
                PostMessageW(hwnd, msg, virtual_key_code as usize, lparam as isize);
            }
        }
        
        key_state_insert_kv(virtual_key_code, direction);
    } else {
        let mut flags:u32;
        if direction == PROTOCOL_KEY_DIRECTION::DOWN {
            flags = 0;
        } else {
            flags = KEYEVENTF_KEYUP;
        }

        if scan_code != 0 {
            flags|=KEYEVENTF_SCANCODE;
            if scan_code&0xE000 != 0 {
                flags|=KEYEVENTF_EXTENDEDKEY
            }
            let keyboard_input:KEYBDINPUT = KEYBDINPUT {
                wVk: 0,
                wScan: (scan_code&0x00FF) as u16,
                dwFlags: flags, 
                time: 0,
                dwExtraInfo: 0 };

            let input:INPUT = INPUT {
                r#type:INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: keyboard_input,
                },
            };
            SendInput(1,&input as *const _,size_of::<INPUT>() as i32);
            key_state_insert_kv(virtual_key_code, direction);
        } else if virtual_key_code != 0 {
            let keyboard_input:KEYBDINPUT = KEYBDINPUT {
                wVk: virtual_key_code as u16,
                wScan: 0,
                dwFlags: flags, 
                time: 0,
                dwExtraInfo: 0 };

            let input:INPUT = INPUT {
                r#type:INPUT_KEYBOARD,
                Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                    ki: keyboard_input,
                },
            };
            SendInput(1,&input as *const _,size_of::<INPUT>() as i32);
            key_state_insert_kv(virtual_key_code, direction);
        }
    }
    0
}

pub unsafe fn hide_cursor(block:i32) -> i32 {
0
}

pub unsafe fn block_input_wrapper(block:i32) -> i32 {
    BlockInput(block)
}

//https://learn.microsoft.com/en-us/windows/win32/api/_winmsg/
//https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-mousemove
//https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-postthreadmessagea
//https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid
pub unsafe fn mouse_move_wrapper(ishiddendesktop:bool,x:i32,y:i32) -> i32 {
    if ishiddendesktop {
        //let mut hwnd: HWND = std::ptr::null_mut();
        //EnumWindows(Some(find_browser_window), &mut hwnd as *mut _ as LPARAM);
        let mut hwnd = crate::windows::first_browser_window();
        if hwnd.is_null() {
            println!("mouse move hwnd is null");
            return 0;
        }
        
        let modifiers = get_modifier_state();
        
        println!("mouse move x:{} y:{}",x,y);
        PostMessageW(hwnd, WM_MOUSEMOVE, modifiers, make_lparam(x, y));
    } else {
        // Convert monitor-relative coordinates to virtual screen coordinates
        let offset = crate::get_capture_offset();
        let virtual_x = x + offset.x;
        let virtual_y = y + offset.y;

        // Get virtual screen bounds (covers all monitors)
        let virtual_screen_x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let virtual_screen_y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let virtual_screen_width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let virtual_screen_height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        // Normalize coordinates relative to the entire virtual screen
        // MOUSEEVENTF_ABSOLUTE with MOUSEEVENTF_VIRTUALDESK uses 0-65535 range across all monitors
        let normalized_x = ((virtual_x - virtual_screen_x) * 65535) / virtual_screen_width;
        let normalized_y = ((virtual_y - virtual_screen_y) * 65535) / virtual_screen_height;

        let mouse_input_move:MOUSEINPUT = MOUSEINPUT {
            dx: normalized_x,
            dy: normalized_y,
            mouseData: 0,
            dwFlags: MOUSEEVENTF_ABSOLUTE|MOUSEEVENTF_VIRTUALDESK|MOUSEEVENTF_MOVE, 
            time: 0,
            dwExtraInfo: 0 };

        let input: INPUT = INPUT {
            r#type:INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: mouse_input_move,
            },
        };
        return SendInput(1,&input as *const _,size_of::<INPUT>() as i32) as i32;
    }
    0
}
