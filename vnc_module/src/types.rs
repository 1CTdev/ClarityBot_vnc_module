/*
send key
+------------------+----------+-----------+
| 1 byte           | 4 bytes  | 1 bytes   |
+------------------+----------+-----------+
| virtual_key_code | scancode | direction |
+------------------+----------+-----------+
*/

use windows_sys::Win32::Foundation::RECT;

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct KEYBOARD_INPUT_PACKET {
    pub virtual_key_code:u8,
    pub scancode:u32,
    pub direction:PROTOCOL_KEY_DIRECTION,
}

#[derive(Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum PROTOCOL_KEY_DIRECTION {
    UP = 0,
    DOWN = 1,
}

impl Default for PROTOCOL_KEY_DIRECTION {
    fn default() -> Self {
        PROTOCOL_KEY_DIRECTION::UP
    }
}


/*
mouse button
+------------------+----------+-----------+----------+----------+
| 1 byte           | 4 bytes  | 1 bytes   | 4 bytes  | 4 bytes  |
+------------------+----------+-----------+----------+----------+
| virtual_key_code | scancode | direction | x offset | y offset |
+------------------+----------+-----------+----------+----------+
*/

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct MOUSE_BUTTON_PACKET {
    pub virtual_key_code:u8,
    pub scancode:u32,
    pub direction:PROTOCOL_KEY_DIRECTION,
    pub x_offset:i32,
    pub y_offset:i32,
}


#[derive(Clone)]
pub struct MONITOR_INFO {
    pub name: String,
    pub device_name: String,   // Monitor device path (e.g., "\\\\.\\DISPLAY1\\Monitor0")
    pub device_string: String, // Human-readable name (e.g., "Generic PnP Monitor")
    pub device_id: String,     // Device ID containing model info (e.g., "MONITOR\\DELA0D8\\...")
    pub model_name: String,    // Parsed from EDID (e.g., "DELL U2718Q")
    pub manufacturer: String,  // 3-letter manufacturer code from EDID (e.g., "DEL")
    pub state_flags: u32,      // StateFlags from DISPLAY_DEVICE
    pub is_virtual: bool,      // True if no physical monitor attached or mirroring driver
    pub is_primary: bool,
    pub monitor_rect: RECT,
    pub work_rect: RECT,
}

/*
mouse move
+----------+----------+
| 4 bytes  | 4 bytes  |
+----------+----------+
| x offset | y offset |
+----------+----------+
*/

#[repr(packed)]
#[derive(Copy, Clone)]
pub struct MOUSE_MOVE_PACKET {
    pub x_offset:i32,
    pub y_offset:i32,
}