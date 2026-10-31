#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::{thread::sleep, time::Duration};
use windows_sys::{
    Wdk::Storage::FileSystem::NtQueryDirectoryFile,
    Win32::{
        Foundation::{CloseHandle, GetLastError, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, STATUS_SUCCESS},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, CREATE_ALWAYS, CreateDirectoryW, CreateFileW, 
            FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_INFO,
            FILE_SHARE_READ, FILE_SHARE_WRITE, FindClose, FindFirstFileW, FindNextFileW,
            GetFileInformationByHandle, GetFileInformationByHandleEx, FileIdInfo,
            GetFinalPathNameByHandleW, GetFullPathNameW, GetLongPathNameW, GetShortPathNameW,
            GetVolumePathNameW, OPEN_EXISTING, WIN32_FIND_DATAW, FILE_NAME_NORMALIZED,
            VOLUME_NAME_DOS, VOLUME_NAME_GUID, VOLUME_NAME_NONE, VOLUME_NAME_NT,
        },
        System::{
            Environment::SetCurrentDirectoryW,
            IO::IO_STATUS_BLOCK,
            LibraryLoader::LoadLibraryW,
            ProcessStatus::GetMappedFileNameW,
            Memory::{CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, PAGE_READONLY},
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

// FileDirectoryInformation class = 1
const FileDirectoryInformation: i32 = 1;

fn main() {
    unsafe {
        let status = LoadLibraryW(w!(r"C:\Users\user\source\repos\rust_hooker\target\debug\rust_hooker.dll"));
        println!("{:x}",status as usize);
        sleep(Duration::from_secs(5));

        println!("\n========================================");
        println!("  TRANSPARENT REDIRECTION DETECTION TESTS");
        println!("========================================\n");
        
        // Original tests
        create_file_tests();
        create_directory_test();
        list_directory_test();
        
        // New detection tests
        detection_test_file_id_comparison();
        detection_test_path_formats();
        detection_test_get_file_information();
        detection_test_short_long_path();
        detection_test_volume_path();
        detection_test_mapped_file();
        detection_test_nt_query_directory_file();
        detection_test_full_path_name();
        detection_test_cross_open_comparison();

        println!("\n=== All tests complete ===");
    }
}


fn list_directory_test() {
    unsafe {
        // List directory contents using absolute path
        println!("\n=== FindFirstFileW/FindNextFileW absolute path test ===");
        let search_path = r#"C:\Users\user\AppData\Local\Microsoft\Edge\*"#;
        println!("Search path: {}", search_path);
        
        let mut find_data: WIN32_FIND_DATAW = std::mem::zeroed();
        let handle = FindFirstFileW(
            w!(r#"C:\Users\user\AppData\Local\Microsoft\Edge\*"#),
            &mut find_data,
        );
        if handle != INVALID_HANDLE_VALUE {
            println!("Directory contents:");
            loop {
                let filename = String::from_utf16_lossy(&find_data.cFileName)
                    .trim_end_matches('\0')
                    .to_string();
                println!("  {}", filename);
                if FindNextFileW(handle, &mut find_data) == 0 {
                    break;
                }
            }
            FindClose(handle);
            
            // Verify by opening the parent directory and checking its path
            let dir_handle = CreateFileW(
                w!(r#"C:\Users\user\AppData\Local\Microsoft\Edge"#),
                GENERIC_READ,
                FILE_SHARE_READ,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            );
            if dir_handle != INVALID_HANDLE_VALUE {
                let queried_path = get_path_from_handle(dir_handle);
                println!("Directory queried path: {}", queried_path);
                if queried_path.to_lowercase().contains("edge") {
                    println!("✓ PASS: Listed directory path contains 'Edge'");
                } else {
                    println!("✗ FAIL: Listed directory path does not contain 'Edge'");
                }
                CloseHandle(dir_handle);
            }
        } else {
            print_last_error(Some(String::from("FindFirstFileW absolute")));
        }

        // List directory contents using relative path
        println!("\n=== FindFirstFileW/FindNextFileW relative path test ===");
        let search_path_rel = r#".\AppData\Local\Microsoft\Edge\*"#;
        println!("Search path: {}", search_path_rel);
        
        let mut find_data: WIN32_FIND_DATAW = std::mem::zeroed();
        let handle = FindFirstFileW(
            w!(r#".\AppData\Local\Microsoft\Edge\*"#),
            &mut find_data,
        );
        if handle != INVALID_HANDLE_VALUE {
            println!("Directory contents:");
            loop {
                let filename = String::from_utf16_lossy(&find_data.cFileName)
                    .trim_end_matches('\0')
                    .to_string();
                println!("  {}", filename);
                if FindNextFileW(handle, &mut find_data) == 0 {
                    break;
                }
            }
            FindClose(handle);
            
            // Verify by opening the parent directory and checking its path
            let dir_handle = CreateFileW(
                w!(r#".\AppData\Local\Microsoft\Edge"#),
                GENERIC_READ,
                FILE_SHARE_READ,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            );
            if dir_handle != INVALID_HANDLE_VALUE {
                let queried_path = get_path_from_handle(dir_handle);
                println!("Directory queried path: {}", queried_path);
                if queried_path.to_lowercase().contains("edge") {
                    println!("✓ PASS: Listed directory path contains 'Edge'");
                } else {
                    println!("✗ FAIL: Listed directory path does not contain 'Edge'");
                }
                CloseHandle(dir_handle);
            }
        } else {
            print_last_error(Some(String::from("FindFirstFileW relative")));
        }
    }
}

fn create_directory_test() {
    unsafe {
        // Create directory using absolute path
        println!("\n=== CreateDirectoryW absolute path test ===");
        let dir_path_abs = w!(r#"C:\Users\user\AppData\Local\Microsoft\Edge\test_dir_absolute"#);
        let status = CreateDirectoryW(dir_path_abs, std::ptr::null());
        print_last_error(Some(String::from("CreateDirectoryW absolute")));
        
        // Open the directory to get a handle and verify the path
        let handle = CreateFileW(
            dir_path_abs,
            GENERIC_READ,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS, // Required to open a directory
            std::ptr::null_mut(),
        );
        
        if handle != INVALID_HANDLE_VALUE {
            let queried_path = get_path_from_handle(handle);
            println!("Input path:    C:\\Users\\user\\AppData\\Local\\Microsoft\\Edge\\test_dir_absolute");
            println!("Queried path:  {}", queried_path);
            if queried_path.to_lowercase().contains("edge") {
                println!("✓ PASS: Directory path contains 'Edge' as expected from caller perspective");
            } else {
                println!("✗ FAIL: Directory path does not contain 'Edge' - hook may be exposing redirected path");
            }
            CloseHandle(handle);
        } else {
            print_last_error(Some(String::from("Open directory for path query")));
            println!("CreateDirectoryW status: {:x}", status as usize);
        }

        // Create directory using relative path
        println!("\n=== CreateDirectoryW relative path test ===");
        let dir_path_rel = w!(r#".\AppData\Local\Microsoft\Edge\test_dir_relative"#);
        let status = CreateDirectoryW(dir_path_rel, std::ptr::null());
        print_last_error(Some(String::from("CreateDirectoryW relative")));
        
        // Open the directory to get a handle and verify the path
        let handle = CreateFileW(
            dir_path_rel,
            GENERIC_READ,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        );
        
        if handle != INVALID_HANDLE_VALUE {
            let queried_path = get_path_from_handle(handle);
            println!("Input path:    .\\AppData\\Local\\Microsoft\\Edge\\test_dir_relative");
            println!("Queried path:  {}", queried_path);
            if queried_path.to_lowercase().contains("edge") {
                println!("✓ PASS: Directory path contains 'Edge' as expected from caller perspective");
            } else {
                println!("✗ FAIL: Directory path does not contain 'Edge' - hook may be exposing redirected path");
            }
            CloseHandle(handle);
        } else {
            print_last_error(Some(String::from("Open directory for path query")));
            println!("CreateDirectoryW status: {:x}", status as usize);
        }
    }
}

fn create_file_tests() {
    unsafe {
        let expected_path = r#"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt"#;
        
        // Test for absolute path
        println!("\n=== CreateFileW absolute path test ===");
        let handle = CreateFileW(
            w!(r#"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt"#),
            GENERIC_READ | GENERIC_WRITE,
            0,
            std::ptr::null(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        print_last_error(Some(String::from("CreateFileW absolute")));
        
        if handle != INVALID_HANDLE_VALUE {
            let queried_path = get_path_from_handle(handle);
            println!("Expected path: {}", expected_path);
            println!("Queried path:  {}", queried_path);
            if queried_path.to_lowercase().contains("edge") {
                println!("✓ PASS: Path contains 'Edge' as expected from caller perspective");
            } else {
                println!("✗ FAIL: Path does not contain 'Edge' - hook may be exposing redirected path");
            }
            CloseHandle(handle);
        } else {
            println!("Failed to create file");
        }

        // Test for path that should NOT match
        println!("\n=== CreateFileW non-matching path test ===");
        let handle = CreateFileW(
            w!(r#"C:\temp\not_match_1.txt"#),
            GENERIC_READ | GENERIC_WRITE,
            0,
            std::ptr::null(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        print_last_error(Some(String::from("CreateFileW non-match")));
        
        if handle != INVALID_HANDLE_VALUE {
            let queried_path = get_path_from_handle(handle);
            println!("Expected path: C:\\temp\\not_match_1.txt");
            println!("Queried path:  {}", queried_path);
            if queried_path.to_lowercase().contains("temp") {
                println!("✓ PASS: Non-matching path was not redirected");
            } else {
                println!("✗ FAIL: Non-matching path was unexpectedly modified");
            }
            CloseHandle(handle);
        } else {
            println!("Failed to create file (this may be expected if C:\\temp doesn't exist)");
        }
        
        // Test for relative path
        SetCurrentDirectoryW(w!(r#"C:\Users\user\"#));
        
        println!("\n=== CreateFileW relative path test ===");
        let expected_relative = r#".\AppData\Local\Microsoft\Edge\match_2.txt"#;
        let handle = CreateFileW(
            w!(r#".\AppData\Local\Microsoft\Edge\match_2.txt"#),
            GENERIC_READ | GENERIC_WRITE,
            0,
            std::ptr::null(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        print_last_error(Some(String::from("CreateFileW relative")));
        
        if handle != INVALID_HANDLE_VALUE {
            let queried_path = get_path_from_handle(handle);
            println!("Input path:    {}", expected_relative);
            println!("Queried path:  {}", queried_path);
            if queried_path.to_lowercase().contains("edge") {
                println!("✓ PASS: Path contains 'Edge' as expected from caller perspective");
            } else {
                println!("✗ FAIL: Path does not contain 'Edge' - hook may be exposing redirected path");
            }
            CloseHandle(handle);
        } else {
            println!("Failed to create file");
        }
    }
}

fn get_path_from_handle(handle: HANDLE) -> String {
    unsafe {
        let mut buffer = [0u16; 512];
        let len = GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0);
        if len > 0 && len < buffer.len() as u32 {
            String::from_utf16_lossy(&buffer[..len as usize])
        } else {
            String::from("<failed to get path>")
        }
    }
}

// ============================================================================
// DETECTION TEST FUNCTIONS
// These attempt to detect if file redirection is occurring
// ============================================================================

/// Test 1: Compare File IDs when opening via "Edge" vs "dldr" paths
/// If they point to the same underlying file, the File IDs should match
fn detection_test_file_id_comparison() {
    println!("\n=== DETECTION TEST: File ID Comparison ===");
    println!("Opening same file via 'Edge' and 'dldr' paths to compare file IDs...\n");
    
    unsafe {
        let edge_path = w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt");
        let dldr_path = w!(r"C:\Users\user\AppData\Local\Microsoft\dldr\match_1.txt");
        
        let h1 = CreateFileW(edge_path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, 
            std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        let h2 = CreateFileW(dldr_path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, 
            std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        
        if h1 != INVALID_HANDLE_VALUE && h2 != INVALID_HANDLE_VALUE {
            let mut info1: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            let mut info2: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            
            GetFileInformationByHandle(h1, &mut info1);
            GetFileInformationByHandle(h2, &mut info2);
            
            println!("  Edge path - Volume: {:08x}, FileIndex: {:08x}{:08x}", 
                info1.dwVolumeSerialNumber, info1.nFileIndexHigh, info1.nFileIndexLow);
            println!("  dldr path - Volume: {:08x}, FileIndex: {:08x}{:08x}", 
                info2.dwVolumeSerialNumber, info2.nFileIndexHigh, info2.nFileIndexLow);
            
            if info1.nFileIndexHigh == info2.nFileIndexHigh && 
               info1.nFileIndexLow == info2.nFileIndexLow &&
               info1.dwVolumeSerialNumber == info2.dwVolumeSerialNumber {
                println!("  ⚠ DETECTION: File IDs match - files are the same (redirection detected)");
            } else {
                println!("  ✓ File IDs differ - appears to be different files");
            }
            
            CloseHandle(h1);
            CloseHandle(h2);
        } else {
            println!("  Could not open both files for comparison");
            if h1 != INVALID_HANDLE_VALUE { CloseHandle(h1); }
            if h2 != INVALID_HANDLE_VALUE { CloseHandle(h2); }
        }
    }
}

/// Test 2: Compare different path format outputs from GetFinalPathNameByHandleW
fn detection_test_path_formats() {
    println!("\n=== DETECTION TEST: GetFinalPathNameByHandleW Path Formats ===");
    println!("Testing different output formats (DOS, GUID, NT, NONE)...\n");
    
    unsafe {
        let handle = CreateFileW(
            w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt"),
            GENERIC_READ, FILE_SHARE_READ, std::ptr::null(), OPEN_EXISTING, 
            FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        
        if handle != INVALID_HANDLE_VALUE {
            let mut buffer = [0u16; 512];
            
            // VOLUME_NAME_DOS (0) - default, should show drive letter path
            let len = GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), 512, VOLUME_NAME_DOS | FILE_NAME_NORMALIZED);
            let dos_path = if len > 0 { String::from_utf16_lossy(&buffer[..len as usize]) } else { String::from("<failed>") };
            
            // VOLUME_NAME_GUID (1) - shows volume GUID path
            buffer.fill(0);
            let len = GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), 512, VOLUME_NAME_GUID | FILE_NAME_NORMALIZED);
            let guid_path = if len > 0 { String::from_utf16_lossy(&buffer[..len as usize]) } else { String::from("<failed>") };
            
            // VOLUME_NAME_NT (2) - shows NT device path
            buffer.fill(0);
            let len = GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), 512, VOLUME_NAME_NT | FILE_NAME_NORMALIZED);
            let nt_path = if len > 0 { String::from_utf16_lossy(&buffer[..len as usize]) } else { String::from("<failed>") };
            
            // VOLUME_NAME_NONE (4) - shows just the path portion
            buffer.fill(0);
            let len = GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), 512, VOLUME_NAME_NONE | FILE_NAME_NORMALIZED);
            let none_path = if len > 0 { String::from_utf16_lossy(&buffer[..len as usize]) } else { String::from("<failed>") };
            
            println!("  DOS path:  {}", dos_path);
            println!("  GUID path: {}", guid_path);
            println!("  NT path:   {}", nt_path);
            println!("  NONE path: {}", none_path);
            
            // Check for detection
            let has_dldr = dos_path.to_lowercase().contains("dldr") || 
                          guid_path.to_lowercase().contains("dldr") ||
                          nt_path.to_lowercase().contains("dldr") ||
                          none_path.to_lowercase().contains("dldr");
            
            if has_dldr {
                println!("  ⚠ DETECTION: 'dldr' found in path output - redirection exposed!");
            } else if dos_path.to_lowercase().contains("edge") {
                println!("  ✓ HIDDEN: All paths show 'Edge' - redirection is transparent");
            }
            
            CloseHandle(handle);
        } else {
            println!("  Could not open file");
        }
    }
}

/// Test 3: GetFileInformationByHandle / GetFileInformationByHandleEx
fn detection_test_get_file_information() {
    println!("\n=== DETECTION TEST: GetFileInformationByHandle(Ex) ===");
    println!("Testing extended file information APIs...\n");
    
    unsafe {
        let handle = CreateFileW(
            w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt"),
            GENERIC_READ, FILE_SHARE_READ, std::ptr::null(), OPEN_EXISTING, 
            FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        
        if handle != INVALID_HANDLE_VALUE {
            // Basic info
            let mut basic_info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            if GetFileInformationByHandle(handle, &mut basic_info) != 0 {
                println!("  BY_HANDLE_FILE_INFORMATION:");
                println!("    Volume Serial: {:08x}", basic_info.dwVolumeSerialNumber);
                println!("    File Index:    {:08x}{:08x}", basic_info.nFileIndexHigh, basic_info.nFileIndexLow);
                println!("    Attributes:    {:08x}", basic_info.dwFileAttributes);
                println!("    Link Count:    {}", basic_info.nNumberOfLinks);
            }
            
            // FileIdInfo - 128-bit file ID (if available)
            let mut file_id_info: FILE_ID_INFO = std::mem::zeroed();
            if GetFileInformationByHandleEx(handle, FileIdInfo, 
                &mut file_id_info as *mut _ as *mut _, std::mem::size_of::<FILE_ID_INFO>() as u32) != 0 {
                println!("  FILE_ID_INFO:");
                println!("    VolumeSerialNumber: {:016x}", file_id_info.VolumeSerialNumber);
                print!("    FileId (128-bit):   ");
                for b in &file_id_info.FileId.Identifier {
                    print!("{:02x}", b);
                }
                println!();
            }
            
            CloseHandle(handle);
        } else {
            println!("  Could not open file");
        }
    }
}

/// Test 4: GetShortPathName / GetLongPathName
fn detection_test_short_long_path() {
    println!("\n=== DETECTION TEST: Short/Long Path Conversion ===");
    println!("Testing GetShortPathNameW and GetLongPathNameW...\n");
    
    unsafe {
        let input_path = w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt");
        let mut short_buffer = [0u16; 512];
        let mut long_buffer = [0u16; 512];
        
        let short_len = GetShortPathNameW(input_path, short_buffer.as_mut_ptr(), 512);
        let short_path = if short_len > 0 && short_len < 512 { 
            String::from_utf16_lossy(&short_buffer[..short_len as usize]) 
        } else { 
            String::from("<failed>") 
        };
        
        let long_len = GetLongPathNameW(input_path, long_buffer.as_mut_ptr(), 512);
        let long_path = if long_len > 0 && long_len < 512 { 
            String::from_utf16_lossy(&long_buffer[..long_len as usize]) 
        } else { 
            String::from("<failed>") 
        };
        
        println!("  Input path: C:\\Users\\user\\AppData\\Local\\Microsoft\\Edge\\match_1.txt");
        println!("  Short path: {}", short_path);
        println!("  Long path:  {}", long_path);
        
        if short_path.to_lowercase().contains("dldr") || long_path.to_lowercase().contains("dldr") {
            println!("  ⚠ DETECTION: 'dldr' exposed in short/long path conversion!");
        } else if long_path.to_lowercase().contains("edge") {
            println!("  ✓ HIDDEN: Path conversions show 'Edge'");
        }
    }
}

/// Test 5: GetVolumePathName
fn detection_test_volume_path() {
    println!("\n=== DETECTION TEST: GetVolumePathNameW ===");
    println!("Testing volume path resolution...\n");
    
    unsafe {
        let input_path = w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt");
        let mut buffer = [0u16; 512];
        
        if GetVolumePathNameW(input_path, buffer.as_mut_ptr(), 512) != 0 {
            let vol_path = String::from_utf16_lossy(&buffer).trim_end_matches('\0').to_string();
            println!("  Input:       C:\\Users\\user\\AppData\\Local\\Microsoft\\Edge\\match_1.txt");
            println!("  Volume path: {}", vol_path);
        } else {
            println!("  GetVolumePathNameW failed");
        }
    }
}

/// Test 6: Memory-mapped file detection via GetMappedFileNameW
fn detection_test_mapped_file() {
    println!("\n=== DETECTION TEST: GetMappedFileNameW ===");
    println!("Testing memory-mapped file name query...\n");
    
    unsafe {
        let handle = CreateFileW(
            w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt"),
            GENERIC_READ, FILE_SHARE_READ, std::ptr::null(), OPEN_EXISTING, 
            FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        
        if handle != INVALID_HANDLE_VALUE {
            // Create file mapping
            let mapping = CreateFileMappingW(handle, std::ptr::null(), PAGE_READONLY, 0, 0, std::ptr::null());
            if !mapping.is_null() {
                let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 1);
                if !view.Value.is_null() {
                    let mut buffer = [0u16; 512];
                    let len = GetMappedFileNameW(
                        std::mem::transmute::<isize, *mut std::ffi::c_void>(-1isize), // current process
                        view.Value, 
                        buffer.as_mut_ptr(), 
                        512
                    );
                    
                    if len > 0 {
                        let mapped_name = String::from_utf16_lossy(&buffer[..len as usize]);
                        println!("  Mapped file name: {}", mapped_name);
                        
                        if mapped_name.to_lowercase().contains("dldr") {
                            println!("  ⚠ DETECTION: 'dldr' exposed in mapped file name!");
                        } else if mapped_name.to_lowercase().contains("edge") {
                            println!("  ✓ HIDDEN: Mapped name shows 'Edge'");
                        }
                    } else {
                        println!("  GetMappedFileNameW returned 0");
                        print_last_error(Some("GetMappedFileNameW".to_string()));
                    }
                    
                    UnmapViewOfFile(view);
                }
                CloseHandle(mapping);
            }
            CloseHandle(handle);
        } else {
            println!("  Could not open file");
        }
    }
}

/// Test 7: NtQueryDirectoryFile - raw NT API for directory listing
fn detection_test_nt_query_directory_file() {
    println!("\n=== DETECTION TEST: NtQueryDirectoryFile ===");
    println!("Testing raw NT directory query API...\n");
    
    unsafe {
        // Open the Edge directory
        let handle = CreateFileW(
            w!(r"C:\Users\user\AppData\Local\Microsoft\Edge"),
            GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), 
            OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, std::ptr::null_mut());
        
        if handle != INVALID_HANDLE_VALUE {
            let mut io_status: IO_STATUS_BLOCK = std::mem::zeroed();
            // Use a larger buffer with proper alignment
            #[repr(C, align(8))]
            struct AlignedBuffer([u8; 8192]);
            let mut buffer = AlignedBuffer([0u8; 8192]);
            
            let status = NtQueryDirectoryFile(
                handle,
                std::ptr::null_mut(), // Event
                None, // ApcRoutine
                std::ptr::null_mut(), // ApcContext
                &mut io_status,
                buffer.0.as_mut_ptr() as *mut _,
                buffer.0.len() as u32,
                FileDirectoryInformation,
                false, // ReturnSingleEntry = FALSE
                std::ptr::null(), // FileName filter
                true, // RestartScan = TRUE
            );
            
            println!("  NtQueryDirectoryFile status: 0x{:08x}", status as u32);
            
            if status == STATUS_SUCCESS {
                // FILE_DIRECTORY_INFORMATION structure layout:
                // 0x00: NextEntryOffset (u32)
                // 0x04: FileIndex (u32)  
                // 0x08: CreationTime (i64)
                // 0x10: LastAccessTime (i64)
                // 0x18: LastWriteTime (i64)
                // 0x20: ChangeTime (i64)
                // 0x28: EndOfFile (i64)
                // 0x30: AllocationSize (i64)
                // 0x38: FileAttributes (u32)
                // 0x3C: FileNameLength (u32) - offset 60
                // 0x40: FileName[1] - offset 64
                
                let mut offset = 0usize;
                println!("  Directory entries:");
                
                while offset < buffer.0.len() - 64 {
                    let entry = &buffer.0[offset..];
                    let next_offset = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]) as usize;
                    let file_name_length = u32::from_le_bytes([entry[60], entry[61], entry[62], entry[63]]) as usize;
                    
                    if file_name_length > 0 && file_name_length < 1024 && offset + 64 + file_name_length <= buffer.0.len() {
                        let name_slice = &entry[64..64 + file_name_length];
                        let name_u16: Vec<u16> = name_slice.chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let name = String::from_utf16_lossy(&name_u16);
                        println!("    {}", name);
                    }
                    
                    if next_offset == 0 { break; }
                    offset += next_offset;
                }
            }
            
            CloseHandle(handle);
        } else {
            println!("  Could not open directory");
        }
    }
}

/// Test 8: GetFullPathNameW
fn detection_test_full_path_name() {
    println!("\n=== DETECTION TEST: GetFullPathNameW ===");
    println!("Testing path canonicalization...\n");
    
    unsafe {
        // First set current directory
        SetCurrentDirectoryW(w!(r"C:\Users\user\"));
        
        let relative_path = w!(r".\AppData\Local\Microsoft\Edge\match_1.txt");
        let mut buffer = [0u16; 512];
        let mut file_part: *mut u16 = std::ptr::null_mut();
        
        let len = GetFullPathNameW(relative_path, 512, buffer.as_mut_ptr(), &mut file_part);
        
        if len > 0 && len < 512 {
            let full_path = String::from_utf16_lossy(&buffer[..len as usize]);
            println!("  Relative input: .\\AppData\\Local\\Microsoft\\Edge\\match_1.txt");
            println!("  Full path:      {}", full_path);
            
            if full_path.to_lowercase().contains("dldr") {
                println!("  ⚠ DETECTION: 'dldr' exposed in GetFullPathNameW!");
            } else if full_path.to_lowercase().contains("edge") {
                println!("  ✓ HIDDEN: Full path shows 'Edge'");
            }
        }
    }
}

/// Test 9: Cross-open comparison - open via Edge path, query, compare to direct dldr open
fn detection_test_cross_open_comparison() {
    println!("\n=== DETECTION TEST: Cross-Open File Size/Time Comparison ===");
    println!("Comparing file attributes from Edge vs dldr paths...\n");
    
    unsafe {
        let h_edge = CreateFileW(
            w!(r"C:\Users\user\AppData\Local\Microsoft\Edge\match_1.txt"),
            GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), 
            OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        
        let h_dldr = CreateFileW(
            w!(r"C:\Users\user\AppData\Local\Microsoft\dldr\match_1.txt"),
            GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), 
            OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        
        if h_edge != INVALID_HANDLE_VALUE && h_dldr != INVALID_HANDLE_VALUE {
            let mut info_edge: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            let mut info_dldr: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            
            GetFileInformationByHandle(h_edge, &mut info_edge);
            GetFileInformationByHandle(h_dldr, &mut info_dldr);
            
            println!("  Edge path file:");
            println!("    Size:        {} bytes", 
                ((info_edge.nFileSizeHigh as u64) << 32) | info_edge.nFileSizeLow as u64);
            println!("    Create time: {:016x}", 
                ((info_edge.ftCreationTime.dwHighDateTime as u64) << 32) | info_edge.ftCreationTime.dwLowDateTime as u64);
            println!("    File Index:  {:08x}{:08x}", info_edge.nFileIndexHigh, info_edge.nFileIndexLow);
            
            println!("  dldr path file:");
            println!("    Size:        {} bytes", 
                ((info_dldr.nFileSizeHigh as u64) << 32) | info_dldr.nFileSizeLow as u64);
            println!("    Create time: {:016x}", 
                ((info_dldr.ftCreationTime.dwHighDateTime as u64) << 32) | info_dldr.ftCreationTime.dwLowDateTime as u64);
            println!("    File Index:  {:08x}{:08x}", info_dldr.nFileIndexHigh, info_dldr.nFileIndexLow);
            
            let same_file = info_edge.nFileIndexHigh == info_dldr.nFileIndexHigh &&
                           info_edge.nFileIndexLow == info_dldr.nFileIndexLow &&
                           info_edge.dwVolumeSerialNumber == info_dldr.dwVolumeSerialNumber;
            
            if same_file {
                println!("\n  ⚠ DETECTION: Both paths reference the SAME file (identical file index)!");
                println!("    This proves Edge path is being redirected to dldr");
            } else {
                println!("\n  Files appear to be different");
            }
            
            CloseHandle(h_edge);
            CloseHandle(h_dldr);
        } else {
            println!("  Could not open one or both files");
            if h_edge != INVALID_HANDLE_VALUE { CloseHandle(h_edge); }
            if h_dldr != INVALID_HANDLE_VALUE { CloseHandle(h_dldr); }
        }
    }
}