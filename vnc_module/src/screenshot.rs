use image::{ImageEncoder, codecs::jpeg::JpegEncoder};
use windows_sys::Win32::{Foundation::RECT, Graphics::Gdi::{BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject}, Storage::Xps::{PW_CLIENTONLY, PrintWindow}, UI::WindowsAndMessaging::{GetDesktopWindow, GetWindowRect, IsZoomed, PW_RENDERFULLCONTENT, SW_RESTORE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, ShowWindow}};
use std::io::Cursor;
use crate::types::MONITOR_INFO;

const TILE_SIZE: u16 = 60;
const TILE_LENGTH_RGB: usize = (TILE_SIZE as usize) * (TILE_SIZE as usize) * 3;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct TileOffset {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug)]
pub struct DiffResult {
    pub has_changes: bool,
    pub diff_image: Vec<u8>,
    pub tile_offsets: Vec<TileOffset>,
    pub width: u32,
    pub height: u32,
}

static mut LAST_SCREENSHOT: Option<Vec<u8>> = None;
static mut SCREENSHOT_BUFFER: Option<Vec<u8>> = None;
static mut DIFF_IMAGE_BUFFER: Option<Vec<u8>> = None;
static mut TILE_BUFFER_1: [u8; TILE_LENGTH_RGB] = [0; TILE_LENGTH_RGB];
static mut TILE_BUFFER_2: [u8; TILE_LENGTH_RGB] = [0; TILE_LENGTH_RGB];

pub unsafe fn browser_screenshot(viewer_width:u32,viewer_height:u32) -> Option<(Vec<u8>, u32, u32)> {
    let mut window_hwnd = crate::windows::first_browser_window();

    let mut window_rect = RECT {left:0,top:0,right:0,bottom:0};
    GetWindowRect(window_hwnd, &mut window_rect);

    let window_width = window_rect.right - window_rect.left;
    let window_height = window_rect.bottom - window_rect.top;

    if window_width > viewer_width as i32 || window_height > viewer_height as i32 {
        // Restore the window if it's maximized, otherwise SetWindowPos won't resize it
        if IsZoomed(window_hwnd) != 0 {
            ShowWindow(window_hwnd, SW_RESTORE);
        }
        SetWindowPos(window_hwnd, std::ptr::null_mut(), 0, 0, viewer_width as i32, viewer_height as i32, SWP_NOZORDER | SWP_SHOWWINDOW);

    }

    let mut window_rect = RECT {left:0,top:0,right:0,bottom:0};
    GetWindowRect(window_hwnd, &mut window_rect);

    let window_width = window_rect.right - window_rect.left;
    let window_height = window_rect.bottom - window_rect.top;
    let hdc_window = GetDC(window_hwnd);

    let hdc_compatible = CreateCompatibleDC(hdc_window);
    let bitmap = CreateCompatibleBitmap(hdc_window, window_width, window_height);
    let old_bitmap = SelectObject(hdc_compatible, bitmap);

    PrintWindow(window_hwnd, hdc_compatible, PW_CLIENTONLY|PW_RENDERFULLCONTENT);

    let header = CreateBitmapHeader(window_width, window_height);
    
    let row_size = ((window_width * 3 + 3) / 4) * 4;
    let bitmap_size = (row_size * window_height) as usize;
    
    if SCREENSHOT_BUFFER.is_none() || SCREENSHOT_BUFFER.as_ref().unwrap().len() < bitmap_size {
        SCREENSHOT_BUFFER = Some(vec![0; bitmap_size]);
    }
    let buf = SCREENSHOT_BUFFER.as_mut().unwrap();
    buf.resize(bitmap_size, 0);

    let lines_copied = GetDIBits(
        hdc_window, 
        bitmap, 
        0, 
        window_height as u32, 
        buf.as_mut_ptr() as *mut _, 
        &header as *const _ as *mut _, 
        0
    );

    SelectObject(hdc_compatible, old_bitmap);
    ReleaseDC(window_hwnd, hdc_window);
    DeleteDC(hdc_compatible);
    DeleteObject(bitmap);

    if lines_copied == 0 {
        eprintln!("browser GetDIBits failed");
        return None;
    }

    convert_bgr_to_rgb(buf, window_width as usize, window_height as usize);

    Some((buf.to_vec(), window_width as u32, window_height as u32))
}

pub unsafe fn screenshot(monitor_info: MONITOR_INFO,viewer_width:u32, viewer_height:u32) -> Option<(Vec<u8>, u32, u32)> {
    let hwnd = GetDesktopWindow();
    let hdc_monitor = GetDC(hwnd);

    let monitor_width = monitor_info.monitor_rect.right - monitor_info.monitor_rect.left;
    let monitor_height = monitor_info.monitor_rect.bottom - monitor_info.monitor_rect.top;

    let hdc_compatible = CreateCompatibleDC(hdc_monitor);
    let bitmap = CreateCompatibleBitmap(hdc_monitor, monitor_width, monitor_height);
    let old_bitmap = SelectObject(hdc_compatible, bitmap);

    let status = BitBlt(
        hdc_compatible, 0, 0, monitor_width, monitor_height,
        hdc_monitor, monitor_info.monitor_rect.left, monitor_info.monitor_rect.top,
        0x00CC0020
    );

    if status == 0 {
        eprintln!("BitBlt failed");
        return None;
    }

    let header = CreateBitmapHeader(monitor_width, monitor_height);
    
    let row_size = ((monitor_width * 3 + 3) / 4) * 4;
    let bitmap_size = (row_size * monitor_height) as usize;
    
    if SCREENSHOT_BUFFER.is_none() || SCREENSHOT_BUFFER.as_ref().unwrap().len() < bitmap_size {
        SCREENSHOT_BUFFER = Some(vec![0; bitmap_size]);
    }

    let buf = SCREENSHOT_BUFFER.as_mut().unwrap();
    buf.resize(bitmap_size, 0);

    let lines_copied = GetDIBits(
        hdc_monitor, 
        bitmap, 
        0, 
        monitor_height as u32, 
        buf.as_mut_ptr() as *mut _, 
        &header as *const _ as *mut _, 
        0
    );

    SelectObject(hdc_compatible, old_bitmap);
    ReleaseDC(hwnd, hdc_monitor);
    DeleteDC(hdc_compatible);
    DeleteObject(bitmap);

    if lines_copied == 0 {
        eprintln!("monitor GetDIBits failed");
        return None;
    }

    convert_bgr_to_rgb(buf, monitor_width as usize, monitor_height as usize);
    
    if viewer_width < monitor_width as u32 || viewer_height < monitor_height as u32 {
        let resized_buf = resize_image(buf, monitor_width as u32, monitor_height as u32, viewer_width, viewer_height);
        return Some((resized_buf, viewer_width, viewer_height));
    }
    
    Some((buf.to_vec(), monitor_width as u32, monitor_height as u32))
}

pub fn place_subrect_rgba(
    rgba_p: &mut [u8],           // destination RGBA buffer
    rgba_c: &[u8],               // source RGBA tile data
    rgba_pw: usize,              // parent (destination) width in pixels
    rgba_ph: usize,              // parent (destination) height in pixels  
    rgba_cw: usize,              // child (source) width in pixels
    rgba_ch: usize,              // child (source) height in pixels
    offset_x: usize,             // x offset in pixels
    offset_y: usize              // y offset in pixels
) {
    let mut offset = (offset_y * rgba_pw + offset_x) * 4;
    let mut sub_index = 0;
    let mut column = 0;
    let mut row = 0;
    
    loop {
        if offset + 3 < rgba_p.len() && sub_index + 3 < rgba_c.len() {
            rgba_p[offset] = rgba_c[sub_index];        
            rgba_p[offset + 1] = rgba_c[sub_index + 1];
            rgba_p[offset + 2] = rgba_c[sub_index + 2];
            rgba_p[offset + 3] = rgba_c[sub_index + 3];
        }
        
        column += 1;
        if column >= rgba_cw {
            offset += (rgba_pw * 4) - (rgba_cw * 4);
            row += 1;
            column = 0;
        }
        
        if row >= rgba_ch {
            break;
        }
        
        offset += 4;
        sub_index += 4;
    }
}


pub fn place_subrect_rgb_optimized(
    rgb_p: &mut [u8],            // destination RGB buffer
    rgb_c: &[u8],                // source RGB tile data
    rgb_pw: usize,               // parent (destination) width in pixels
    rgb_ph: usize,               // parent (destination) height in pixels
    rgb_cw: usize,               // child (source) width in pixels
    rgb_ch: usize,               // child (source) height in pixels
    offset_x: usize,             // x offset in pixels
    offset_y: usize              // y offset in pixels
) {
    let mut offset = (offset_y * rgb_pw + offset_x) * 3;
    let mut sub_index = 0;
    let mut column = 0;
    let mut row = 0;
    
    loop {
        if offset + 2 < rgb_p.len() && sub_index + 2 < rgb_c.len() {
            rgb_p[offset] = rgb_c[sub_index];         
            rgb_p[offset + 1] = rgb_c[sub_index + 1];
            rgb_p[offset + 2] = rgb_c[sub_index + 2];
        }
        
        column += 1;
        if column >= rgb_cw {
            offset += (rgb_pw * 3) - (rgb_cw * 3);
            row += 1;
            column = 0;
        }
        
        if row >= rgb_ch {
            break;
        }
        
        offset += 3;
        sub_index += 3;
    }
}


pub unsafe fn diff_frame_buffers(current_screenshot: Vec<u8>, width: u32, height: u32,keyframe:bool) -> DiffResult {
    // If keyframe requested, flush buffers and send whole frame
    if keyframe {
        LAST_SCREENSHOT = None;
        SCREENSHOT_BUFFER = None;
        DIFF_IMAGE_BUFFER = None;
    }
    
    let reference_buffer = match &LAST_SCREENSHOT {
        Some(ref_buf) => ref_buf,
        None => {
            LAST_SCREENSHOT = Some(current_screenshot.clone());
            return create_full_diff_result_optimized(&current_screenshot, width, height);
        }
    };
    
    let row_size = ((width as usize * 3 + 3) / 4) * 4;
    let required_size = row_size * height as usize;
    if DIFF_IMAGE_BUFFER.is_none() || DIFF_IMAGE_BUFFER.as_ref().unwrap().len() < required_size {
        DIFF_IMAGE_BUFFER = Some(vec![0u8; required_size]);
    }
    let diff_image = DIFF_IMAGE_BUFFER.as_mut().unwrap();
    diff_image.resize(required_size, 0);
    diff_image.fill(0);
    
    let mut tile_offsets = Vec::new();
    let mut has_changes = false;
    
    let mut x: u16 = 0;
    while x < width as u16 {
        let mut y: u16 = 0;
        while y < height as u16 {
            let (tile_x, tile_y) = calculate_tile_position(x, y, width as u16, height as u16);
            
            if tiles_different_direct(reference_buffer, &current_screenshot, width as usize, height as usize, tile_x as usize, tile_y as usize) {
                copy_tile_direct(&current_screenshot, diff_image, width as usize, height as usize, 
                               tile_x as usize, tile_y as usize);
                tile_offsets.push(TileOffset { x: tile_x, y: tile_y });
                has_changes = true;
            }
            
            y += TILE_SIZE;
        }
        x += TILE_SIZE;
    }
    
    if let Some(ref mut last) = LAST_SCREENSHOT {
        if last.len() == current_screenshot.len() {
            last.copy_from_slice(&current_screenshot);
        } else {
            *last = current_screenshot.clone();
        }
    } else {
        LAST_SCREENSHOT = Some(current_screenshot.clone());
    }
    
    DiffResult {
        has_changes,
        diff_image: diff_image.clone(),
        tile_offsets,
        width,
        height,
    }
}

fn calculate_tile_position(x: u16, y: u16, width: u16, height: u16) -> (u16, u16) {
    let tile_x = if x + TILE_SIZE > width {
        width - TILE_SIZE
    } else {
        x
    };
    
    let tile_y = if y + TILE_SIZE > height {
        height - TILE_SIZE
    } else {
        y
    };
    
    (tile_x, tile_y)
}

fn create_full_diff_result(screenshot: Vec<u8>, width: u32, height: u32) -> DiffResult {
    let mut tile_offsets = Vec::new();
    
    let mut x: u16 = 0;
    while x < width as u16 {
        let mut y: u16 = 0;
        while y < height as u16 {
            let (tile_x, tile_y) = calculate_tile_position(x, y, width as u16, height as u16);
            tile_offsets.push(TileOffset { x: tile_x, y: tile_y });
            
            y += TILE_SIZE;
        }
        x += TILE_SIZE;
    }
    
    DiffResult {
        has_changes: true,
        diff_image: screenshot,
        tile_offsets,
        width,
        height,
    }
}

unsafe fn create_full_diff_result_optimized(screenshot: &[u8], width: u32, height: u32) -> DiffResult {
    let mut tile_offsets = Vec::new();
    
    let mut x: u16 = 0;
    while x < width as u16 {
        let mut y: u16 = 0;
        while y < height as u16 {
            let (tile_x, tile_y) = calculate_tile_position(x, y, width as u16, height as u16);
            tile_offsets.push(TileOffset { x: tile_x, y: tile_y });
            
            y += TILE_SIZE;
        }
        x += TILE_SIZE;
    }
    
    DiffResult {
        has_changes: true,
        diff_image: screenshot.to_vec(),
        tile_offsets,
        width,
        height,
    }
}

fn get_subrect_rgb(framebuffer: &[u8], width: usize, height: usize, off_x: usize, off_y: usize) -> Vec<u8> {
    let mut tile_data = Vec::with_capacity(TILE_LENGTH_RGB);
    let row_size = ((width * 3 + 3) / 4) * 4;
    
    for line in 0..TILE_SIZE as usize {
        if off_y + line >= height {
            break;
        }
        
        for column in 0..TILE_SIZE as usize {
            if off_x + column >= width {
                tile_data.extend_from_slice(&[0, 0, 0]);
                continue;
            }
            
            let fb_offset = (off_y + line) * row_size + (off_x + column) * 3;
            if fb_offset + 2 < framebuffer.len() {
                tile_data.push(framebuffer[fb_offset]);
                tile_data.push(framebuffer[fb_offset + 1]);
                tile_data.push(framebuffer[fb_offset + 2]);
            } else {
                tile_data.extend_from_slice(&[0, 0, 0]);
            }
        }
    }
    
    tile_data.resize(TILE_LENGTH_RGB, 0);
    tile_data
}

fn tiles_different(tile1: &[u8], tile2: &[u8]) -> bool {
    if tile1.len() != tile2.len() {
        return true;
    }
    
    for i in 0..tile1.len() {
        if tile1[i] != tile2[i] {
            return true;
        }
    }
    
    false
}

unsafe fn tiles_different_direct(ref_buffer: &[u8], cur_buffer: &[u8], width: usize, height: usize, 
                                 tile_x: usize, tile_y: usize) -> bool {
    let row_size = ((width * 3 + 3) / 4) * 4;
    
    for line in 0..TILE_SIZE as usize {
        if tile_y + line >= height {
            break;
        }
        
        for column in 0..TILE_SIZE as usize {
            if tile_x + column >= width {
                continue;
            }
            
            let offset = (tile_y + line) * row_size + (tile_x + column) * 3;
            if offset + 2 < ref_buffer.len() && offset + 2 < cur_buffer.len() {
                if ref_buffer[offset] != cur_buffer[offset] ||
                   ref_buffer[offset + 1] != cur_buffer[offset + 1] ||
                   ref_buffer[offset + 2] != cur_buffer[offset + 2] {
                    return true;
                }
            }
        }
    }
    
    false
}

unsafe fn copy_tile_direct(src_buffer: &[u8], dest_buffer: &mut [u8], width: usize, height: usize,
                          tile_x: usize, tile_y: usize) {
    let row_size = ((width * 3 + 3) / 4) * 4;
    
    for line in 0..TILE_SIZE as usize {
        if tile_y + line >= height {
            break;
        }
        
        for column in 0..TILE_SIZE as usize {
            if tile_x + column >= width {
                continue;
            }
            
            let offset = (tile_y + line) * row_size + (tile_x + column) * 3;
            if offset + 2 < src_buffer.len() && offset + 2 < dest_buffer.len() {
                dest_buffer[offset] = src_buffer[offset];
                dest_buffer[offset + 1] = src_buffer[offset + 1];
                dest_buffer[offset + 2] = src_buffer[offset + 2];
            }
        }
    }
}

fn place_subrect_rgb(dest_buffer: &mut [u8], tile_data: &[u8], dest_width: usize, dest_height: usize, 
                     tile_width: usize, tile_height: usize, offset_x: usize, offset_y: usize) {
    let dest_row_size = ((dest_width * 3 + 3) / 4) * 4;
    let mut src_index = 0;
    
    for row in 0..tile_height {
        if offset_y + row >= dest_height {
            break;
        }
        
        for col in 0..tile_width {
            if offset_x + col >= dest_width || src_index + 2 >= tile_data.len() {
                break;
            }
            
            let dest_offset = (offset_y + row) * dest_row_size + (offset_x + col) * 3;
            if dest_offset + 2 < dest_buffer.len() {
                dest_buffer[dest_offset] = tile_data[src_index];         
                dest_buffer[dest_offset + 1] = tile_data[src_index + 1];
                dest_buffer[dest_offset + 2] = tile_data[src_index + 2];
            }
            
            src_index += 3;
        }
    }
}

fn convert_bgr_to_rgb(buffer: &mut [u8], width: usize, height: usize) {
    let row_size = ((width * 3 + 3) / 4) * 4;
    
    for y in 0..height {
        for x in 0..width {
            let offset = y * row_size + x * 3;
            if offset + 2 < buffer.len() {
                buffer.swap(offset, offset + 2);
            }
        }
    }
}

pub fn CreateBitmapHeader(width: i32, height: i32) -> BITMAPINFOHEADER {
    BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 24,
        biCompression: 0,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    }
}

pub unsafe fn screenshot_with_diff(monitor_info: Option<MONITOR_INFO>,viewer_width:u32, viewer_height:u32,keyframe:bool) -> Option<DiffResult> {
    let current_screenshot = if monitor_info.is_none() {
        browser_screenshot(viewer_width, viewer_height)
    } else {
        screenshot(monitor_info.unwrap(), viewer_width, viewer_height)
    };
    match current_screenshot {
        Some(current_screenshot) => {
            Some(diff_frame_buffers(current_screenshot.0, current_screenshot.1, current_screenshot.2,keyframe))
        },
        None => {
            None
        }
    }
}

pub unsafe fn reset_last_screenshot() {
    LAST_SCREENSHOT = None;
}

pub unsafe fn reset_all_buffers() {
    LAST_SCREENSHOT = None;
    SCREENSHOT_BUFFER = None;
    DIFF_IMAGE_BUFFER = None;
    TILE_BUFFER_1.fill(0);
    TILE_BUFFER_2.fill(0);
}

pub unsafe fn has_last_screenshot() -> bool {
    LAST_SCREENSHOT.is_some()
}

pub unsafe fn encode_rgb_to_jpeg(rgb_data: &[u8], width: u32, height: u32, quality: u8) -> Result<(Vec<u8>,u32,u32), Box<dyn std::error::Error>> {
    let mut jpeg_buffer = Vec::new();
    let cursor = Cursor::new(&mut jpeg_buffer);
    
    let encoder = JpegEncoder::new_with_quality(cursor, quality);
    
    let row_size = ((width * 3 + 3) / 4) * 4;
    let packed_row = (width * 3) as usize;

    let mut packed_data = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height as usize {
        let row_start = y * row_size as usize;
        packed_data.extend_from_slice(&rgb_data[row_start..row_start + packed_row]);
    }

    
    encoder.write_image(&packed_data, width, height, image::ExtendedColorType::Rgb8)?;
    
    return Ok((jpeg_buffer,width,height));
}

pub fn place_tile_in_composite(composite: &mut [u8], tile_data: &[u8], composite_width: usize, x: usize, y: usize) {
    const TILE_SIZE: usize = 60;
    
    for row in 0..TILE_SIZE {
        for col in 0..TILE_SIZE {
            let tile_offset = (row * TILE_SIZE + col) * 3;
            let composite_offset = ((y + row) * composite_width + (x + col)) * 3;
            
            if tile_offset + 2 < tile_data.len() && composite_offset + 2 < composite.len() {
                composite[composite_offset] = tile_data[tile_offset];
                composite[composite_offset + 1] = tile_data[tile_offset + 1];
                composite[composite_offset + 2] = tile_data[tile_offset + 2];
            }
        }
    }
}

pub fn resize_image(rgb_data: &[u8], width: u32, height: u32, new_width: u32, new_height: u32) -> Vec<u8> {
    let scale_x = width as f32 / new_width as f32;
    let scale_y = height as f32 / new_height as f32;
    let row_size = ((width * 3 + 3) / 4) * 4;
    
    let mut resized_data = Vec::with_capacity((new_width * new_height * 3) as usize);
    
    for thumb_y in 0..new_height {
        for thumb_x in 0..new_width {
            let src_x = (thumb_x as f32 * scale_x) as u32;
            let src_y = (thumb_y as f32 * scale_y) as u32;
            
            if src_x < width && src_y < height {
                let offset = (src_y * row_size + src_x * 3) as usize;
                if offset + 2 < rgb_data.len() {
                    resized_data.push(rgb_data[offset]);
                    resized_data.push(rgb_data[offset + 1]);
                    resized_data.push(rgb_data[offset + 2]);
                } else {
                    resized_data.push(0);
                    resized_data.push(0);
                    resized_data.push(0);
                }
            } else {
                resized_data.push(0);
                resized_data.push(0);
                resized_data.push(0);
            }
        }
    }
    
    resized_data
}

