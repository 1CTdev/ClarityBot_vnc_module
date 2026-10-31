use std::{collections::HashMap, slice};

use windows_sys::{Wdk::{Storage::FileSystem::{FILE_ALL_INFORMATION, FILE_VOLUME_NAME_INFORMATION, FileAllInformation, FileFsVolumeInformation, FileVolumeNameInformation, NtQueryInformationFile, NtQueryVolumeInformationFile}, System::{SystemInformation::NtQuerySystemInformation, SystemServices::FILE_FS_VOLUME_INFORMATION, Threading::NtQueryInformationProcess}}, Win32::{Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_SUCCESS, GetLastError, HANDLE, STATUS_INFO_LENGTH_MISMATCH, UNICODE_STRING}, Storage::FileSystem::QueryDosDeviceW, System::{Diagnostics::Debug::ReadProcessMemory, IO::IO_STATUS_BLOCK, Registry::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE, KEY_READ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW}, Threading::{CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DETACHED_PROCESS, GetCurrentProcess, OpenProcess, PEB, PROCESS_BASIC_INFORMATION, PROCESS_DUP_HANDLE, PROCESS_INFORMATION, PROCESS_QUERY_INFORMATION, PROCESS_SUSPEND_RESUME, PROCESS_VM_READ, STARTUPINFOW}}}, w};

use win_utils::{self, SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX};

//Computer\HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\App Paths\
//Computer\HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\App Paths\

//C:\Users\user\AppData\Roaming\Mozilla\Firefox\Profiles\h32jcrcg.default-release
//C:\Program Files\WindowsApps\Mozilla.Firefox_146.0.1.0_x64__n80bbvh6b1yt2\VFS\ProgramFiles\Firefox Package Root\firefox.exe
//"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
//C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default
//need to get browser paths,profile paths, open browsers, profiles that are currently in use + default browser

//FPS_BROWSER_USER_PROFILE_STRING, Default
//


pub unsafe fn msedge_profile_from_process(pid:u32) -> (String,String) {
    let peb_data = win_utils::unicode_peb_unicode(pid);
    let profile_directory = peb_data.get("EDGE_USER_DATA_DIR").unwrap();

    let profile_name:String = peb_data.get("FPS_BROWSER_USER_PROFILE_STRING").map_or("Default", |v| v).to_string();
    (format!(r#"{}\{}"#,(*profile_directory).clone(),profile_name),profile_name)
}

pub unsafe fn chrome_profile_from_process(pid:u32) -> (String,String) {
    (String::new(),String::new())
}

pub unsafe fn brave_profile_from_process(pid:u32) -> (String,String) {
    (String::new(),String::new())
}

pub unsafe fn firefox_profile_from_process(pid:u32) -> (String,String) {
    let lock_file:&str = r#"\parent.lock"#;
    let peb_data = win_utils::unicode_peb_unicode(pid);
    let profile_name = peb_data.get("FPS_BROWSER_USER_PROFILE_STRING").unwrap();

    let file_index = win_utils::get_object_index("File".to_string());
    let handles = win_utils::open_handles(Some(pid as usize),Some(file_index as u16)).unwrap();
    let test = match_file(pid, handles,String::from(lock_file));
    if test.len() > 0 {
        return (test[0].1.clone().replace(lock_file,""),(*profile_name).clone())
    }
    (String::new(),String::new())
}

pub unsafe fn browser_paths() -> Result<Vec<String>,u32> {
    let mut browsers: Vec<String> = Vec::new();
    for parent_key in [HKEY_LOCAL_MACHINE,HKEY_CURRENT_USER] {

        let mut hkey: HANDLE = std::ptr::null_mut();
        let status = RegOpenKeyExW(parent_key,
        w!(r#"Software\Microsoft\Windows\CurrentVersion\App Paths"#),
        0,
        KEY_READ|KEY_ENUMERATE_SUB_KEYS|KEY_QUERY_VALUE,
        &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return Err(status)
        }
        
        let mut index: u32 = 0;
        loop {
            let mut image_name_buf: Vec<u16> = vec![0u16; 256];
            let mut image_name_len: u32 = 256;
            let status = RegEnumKeyExW(hkey,
            index,
            image_name_buf.as_mut_ptr(),
            &mut image_name_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            );
            if status != ERROR_SUCCESS {
                RegCloseKey(hkey);
                break
            }
            let image_name = String::from_utf16_lossy(&image_name_buf[..image_name_len as usize]);

            if image_name == "msedge.exe" || image_name == "firefox.exe" || image_name == "chrome.exe" || image_name == "brave.exe" {
                let subkey_path_wide: Vec<u16> = image_name.encode_utf16().chain(std::iter::once(0)).collect();
                let mut sub_key:HANDLE = Default::default();
                let status = RegOpenKeyExW(hkey,
                subkey_path_wide.as_ptr(),
                0,
                KEY_READ|KEY_ENUMERATE_SUB_KEYS|KEY_QUERY_VALUE,
                &mut sub_key
                );
                if status != ERROR_SUCCESS {
                    index+=1;
                    continue;
                }

                let mut exe_buf: Vec<u16> = vec![0u16; 512];
                let mut exe_buf_size: u32 = 512 * 2;
                let mut reg_type: u32 = 0;

                let status = RegQueryValueExW(
                    sub_key,
                    std::ptr::null(),
                    std::ptr::null(),
                    &mut reg_type,
                    exe_buf.as_mut_ptr() as *mut _,
                    &mut exe_buf_size,
                );
                if status != ERROR_SUCCESS {
                    index+=1;
                    continue;
                }

                RegCloseKey(sub_key);

                let exe_path = String::from_utf16_lossy(&exe_buf[..(exe_buf_size / 2) as usize])
                .trim_matches('\0')
                .trim_matches('"')
                .to_string();
                browsers.push(exe_path);
            }
            index+=1;
        }
    }
    Ok(browsers)
}

#[derive(Clone, Debug)]
pub struct BrowserProfile {
    pub browser_name: String,
    pub module_path: String,
    pub profile_name: String,
    pub profile_path: String,
    pub is_default: bool,
}

pub unsafe fn browser_profiles() -> Vec<BrowserProfile> {
    let mut profiles: Vec<BrowserProfile> = Vec::new();
    
    let browser_paths_list = browser_paths().unwrap_or_default();
    
    let find_module_path = |browser_name: &str| -> String {
        let exe_name = match browser_name {
            "Chrome" => "chrome.exe",
            "Edge" => "msedge.exe",
            "Brave" => "brave.exe",
            "Firefox" => "firefox.exe",
            _ => "",
        };
        browser_paths_list.iter()
            .find(|p| p.to_lowercase().ends_with(exe_name))
            .cloned()
            .unwrap_or_default()
    };

    enumerate_chromium_profiles(
        w!(r#"Software\Google\Chrome\PreferenceMACs"#),
        "Chrome",
        &find_module_path("Chrome"),
        &mut profiles,
    );

    enumerate_chromium_profiles(
        w!(r#"Software\Microsoft\Edge\PreferenceMACs"#),
        "Edge",
        &find_module_path("Edge"),
        &mut profiles,
    );

    enumerate_chromium_profiles(
        w!(r#"Software\BraveSoftware\Brave-Browser\PreferenceMACs"#),
        "Brave",
        &find_module_path("Brave"),
        &mut profiles,
    );

    enumerate_firefox_profiles(&find_module_path("Firefox"), &mut profiles);

    profiles
}


pub unsafe fn enumerate_chromium_profiles(
    reg_path: *const u16,
    browser_name: &str,
    module_path: &str,
    profiles: &mut Vec<BrowserProfile>,
) {
    let mut hkey: HANDLE = std::ptr::null_mut();
    let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        reg_path,
        0,
        KEY_READ | KEY_ENUMERATE_SUB_KEYS,
        &mut hkey,
    );

    if status != ERROR_SUCCESS {
        return;
    }

    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let local_state_path = match browser_name {
        "Chrome" => format!(r"{}\Google\Chrome\User Data\Local State", local_app_data),
        "Edge" => format!(r"{}\Microsoft\Edge\User Data\Local State", local_app_data),
        "Brave" => format!(r"{}\BraveSoftware\Brave-Browser\User Data\Local State", local_app_data),
        _ => String::new(),
    };
    
    let default_profile = get_chromium_default_profile(&local_state_path);

    let mut index: u32 = 0;
    loop {
        let mut profile_name_buf: Vec<u16> = vec![0u16; 256];
        let mut profile_name_len: u32 = 256;

        let status = RegEnumKeyExW(
            hkey,
            index,
            profile_name_buf.as_mut_ptr(),
            &mut profile_name_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        if status != ERROR_SUCCESS {
            break;
        }

        let profile_name = String::from_utf16_lossy(&profile_name_buf[..profile_name_len as usize]);

        let profile_path = match browser_name {
            "Chrome" => format!(r"{}\Google\Chrome\User Data\{}", local_app_data, profile_name),
            "Edge" => format!(r"{}\Microsoft\Edge\User Data\{}", local_app_data, profile_name),
            "Brave" => format!(r"{}\BraveSoftware\Brave-Browser\User Data\{}", local_app_data, profile_name),
            _ => String::new(),
        };

        let is_default = profile_name == default_profile;

        profiles.push(BrowserProfile {
            browser_name: browser_name.to_string(),
            module_path: module_path.to_string(),
            profile_name: profile_name.clone(),
            profile_path,
            is_default,
        });

        index += 1;
    }

    RegCloseKey(hkey);
}


pub fn get_chromium_default_profile(local_state_path: &str) -> String {
    if let Ok(contents) = std::fs::read_to_string(local_state_path) {
        // Look for "profile.last_used" in the JSON
        // Format: "profile":{"last_used":"Profile 1",...}
        if let Some(profile_start) = contents.find(r#""profile""#) {
            let remaining = &contents[profile_start..];
            if let Some(last_used_start) = remaining.find(r#""last_used""#) {
                let after_key = &remaining[last_used_start + 11..];
                if let Some(colon_pos) = after_key.find(':') {
                    let after_colon = &after_key[colon_pos + 1..];
                    let trimmed = after_colon.trim_start();
                    if trimmed.starts_with('"') {
                        let value_start = &trimmed[1..];
                        if let Some(end_quote) = value_start.find('"') {
                            return value_start[..end_quote].to_string();
                        }
                    }
                }
            }
        }
    }
    "Default".to_string()
}


pub unsafe fn enumerate_firefox_profiles(module_path: &str, profiles: &mut Vec<BrowserProfile>) {
    let module_path_owned = module_path.to_string();
    
    let app_data = std::env::var("APPDATA").unwrap_or_default();
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    
    // Possible profiles.ini locations:
    // 1. Traditional: %APPDATA%\Mozilla\Firefox\profiles.ini
    // 2. WindowsApps: %LOCALAPPDATA%\Packages\Mozilla.Firefox_*\LocalCache\Roaming\Mozilla\Firefox\profiles.ini
    // 3. Alternative: %APPDATA%\Mozilla\Firefox\installs.ini points to profiles
    
    let mut ini_paths: Vec<String> = Vec::new();
    
    ini_paths.push(format!(r"{}\Mozilla\Firefox\profiles.ini", app_data));
    
    let packages_dir = format!(r"{}\Packages", local_app_data);
    if let Ok(entries) = std::fs::read_dir(&packages_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("Mozilla.Firefox") {
                let ini_path = format!(
                    r"{}\{}\LocalCache\Roaming\Mozilla\Firefox\profiles.ini",
                    packages_dir, name
                );
                ini_paths.push(ini_path);
            }
        }
    }
    
    for ini_path in ini_paths {
        if let Ok(contents) = std::fs::read_to_string(&ini_path) {
            parse_firefox_profiles_ini(&contents, &ini_path, &module_path_owned, profiles);
        }
    }
    
    if profiles.iter().filter(|p| p.browser_name == "Firefox").count() == 0 {
        enumerate_firefox_profiles_registry(&module_path_owned, profiles);
    }
}


pub fn parse_firefox_profiles_ini(
    contents: &str,
    ini_path: &str,
    module_path: &str,
    profiles: &mut Vec<BrowserProfile>,
) {
    let base_dir = std::path::Path::new(ini_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    
    let mut current_section = String::new();
    let mut current_name = String::new();
    let mut current_path = String::new();
    let mut is_relative = true;
    let mut is_default = false;
    
    for line in contents.lines() {
        let line = line.trim();
        
        if line.starts_with('[') && line.ends_with(']') {
            if current_section.starts_with("Profile") && !current_path.is_empty() {
                let full_path = if is_relative {
                    format!(r"{}\{}", base_dir, current_path.replace('/', "\\"))
                } else {
                    current_path.replace('/', "\\")
                };
                
                let name = if current_name.is_empty() {
                    current_path.clone()
                } else {
                    current_name.clone()
                };
                
                profiles.push(BrowserProfile {
                    browser_name: "Firefox".to_string(),
                    module_path: module_path.to_string(),
                    profile_name: name,
                    profile_path: full_path,
                    is_default,
                });
            }
            
            current_section = line[1..line.len()-1].to_string();
            current_name.clear();
            current_path.clear();
            is_relative = true;
            is_default = false;
        } else if line.contains('=') {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim();
                let value = parts[1].trim();
                
                match key {
                    "Name" => current_name = value.to_string(),
                    "Path" => current_path = value.to_string(),
                    "IsRelative" => is_relative = value == "1",
                    "Default" => is_default = value == "1",
                    _ => {}
                }
            }
        }
    }
    
    if current_section.starts_with("Profile") && !current_path.is_empty() {
        let full_path = if is_relative {
            format!(r"{}\{}", base_dir, current_path.replace('/', r"\"))
        } else {
            current_path.replace('/', "\\")
        };
        
        let name = if current_name.is_empty() {
            current_path
        } else {
            current_name
        };
        
        profiles.push(BrowserProfile {
            browser_name: "Firefox".to_string(),
            module_path: module_path.to_string(),
            profile_name: name,
            profile_path: full_path,
            is_default,
        });
    }
}


pub unsafe fn enumerate_firefox_profiles_registry(module_path: &str, profiles: &mut Vec<BrowserProfile>) {
    let mut hkey: HANDLE = std::ptr::null_mut();
    let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!(r#"Software\Mozilla\Firefox\Profiles"#),
        0,
        KEY_READ | KEY_ENUMERATE_SUB_KEYS,
        &mut hkey,
    );

    if status != ERROR_SUCCESS {
        return;
    }
    
    let module_path_owned = module_path.to_string();

    let mut index: u32 = 0;
    loop {
        let mut profile_id_buf: Vec<u16> = vec![0u16; 256];
        let mut profile_id_len: u32 = 256;

        let status = RegEnumKeyExW(
            hkey,
            index,
            profile_id_buf.as_mut_ptr(),
            &mut profile_id_len,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        if status != ERROR_SUCCESS {
            break;
        }

        let profile_id = String::from_utf16_lossy(&profile_id_buf[..profile_id_len as usize]);

        let subkey_wide: Vec<u16> = profile_id.encode_utf16().chain(std::iter::once(0)).collect();
        let mut subkey: HANDLE = std::ptr::null_mut();
        let status = RegOpenKeyExW(
            hkey,
            subkey_wide.as_ptr(),
            0,
            KEY_READ | KEY_QUERY_VALUE,
            &mut subkey,
        );

        if status == ERROR_SUCCESS {
            let mut name_buf: Vec<u16> = vec![0u16; 256];
            let mut name_buf_size: u32 = 256 * 2;
            let mut reg_type: u32 = 0;

            let status = RegQueryValueExW(
                subkey,
                w!("Name"),
                std::ptr::null(),
                &mut reg_type,
                name_buf.as_mut_ptr() as *mut _,
                &mut name_buf_size,
            );

            let profile_name = if status == ERROR_SUCCESS && name_buf_size > 0 {
                String::from_utf16_lossy(&name_buf[..(name_buf_size / 2) as usize])
                    .trim_matches('\0')
                    .to_string()
            } else {
                profile_id.clone()
            };

            let mut path_buf: Vec<u16> = vec![0u16; 512];
            let mut path_buf_size: u32 = 512 * 2;

            let status = RegQueryValueExW(
                subkey,
                w!("Path"),
                std::ptr::null(),
                &mut reg_type,
                path_buf.as_mut_ptr() as *mut _,
                &mut path_buf_size,
            );

            let profile_path = if status == ERROR_SUCCESS && path_buf_size > 0 {
                String::from_utf16_lossy(&path_buf[..(path_buf_size / 2) as usize])
                    .trim_matches('\0')
                    .to_string()
            } else {
                let app_data = std::env::var("APPDATA").unwrap_or_default();
                format!(r"{}\Mozilla\Firefox\Profiles\{}", app_data, profile_id)
            };

            profiles.push(BrowserProfile {
                browser_name: "Firefox".to_string(),
                module_path: module_path_owned.clone(),
                profile_name,
                profile_path,
                is_default: false,
            });

            RegCloseKey(subkey);
        }

        index += 1;
    }

    RegCloseKey(hkey);
}

//computer\HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.html\OpenWithList
//Computer\HKEY_CLASSES_ROOT\.html\OpenWithProgids

//Computer\HKEY_LOCAL_MACHINE\SOFTWARE\RegisteredApplications
//Software\Clients\StartMenuInternet\Microsoft Edge\Capabilities

//SOFTWARE\\Clients\\StartMenuInternet
pub unsafe fn default_browser() -> String {
    let mut hkey :HANDLE = Default::default();

    let status = RegOpenKeyExW(HKEY_CLASSES_ROOT,w!(r#"http\shell\open\command"#),0, KEY_READ|KEY_ENUMERATE_SUB_KEYS|KEY_QUERY_VALUE,&mut hkey);
    if status != ERROR_SUCCESS {
        return String::new();
    }
    let mut buf:Vec<u16> = Vec::with_capacity(256);
    let mut buf_size: u32 = 256 * 2;
    let reg_type:u32 = 0;
    let status = RegQueryValueExW(hkey,w!(""),std::ptr::null(),&reg_type as *const _ as *mut _, buf.as_mut_ptr() as *mut _,&mut buf_size as *mut _);
    buf.set_len((buf_size/2) as usize);
    if status != ERROR_SUCCESS {
        return String::new();
    }

    sanitize_default_browser_value(String::from_utf16(&buf).unwrap())
}


pub unsafe fn sanitize_default_browser_value(raw:String) -> String {
    let exe_index = match raw.find(".exe") {
        Some(idx) => idx,
        None => return String::new(),
    };
    
    if raw.starts_with('"') {
        return raw[1..exe_index+4].to_string();
    }

    let path_start = raw[..exe_index]
        .rfind(|c: char| c == ' ' || c == '\t')
        .map(|idx| {
            if idx + 2 < raw.len() && raw.chars().nth(idx + 2) == Some(':') {
                idx + 1
            } else {
                0
            }
        })
        .unwrap_or(0);
    
    raw[path_start..exe_index+4].to_string()
}

 pub unsafe fn match_file(pid:u32,handles:Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX>,file_name:String) -> Vec<(SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX,String)> {
    let target_process_handle = OpenProcess(PROCESS_DUP_HANDLE|PROCESS_QUERY_INFORMATION|PROCESS_SUSPEND_RESUME,0,pid);
    if !win_utils::valid_handle(target_process_handle) {
        let status = GetLastError();
        println!("{:x}",status as usize);
    }

    let my_process_handle = GetCurrentProcess();
    if !win_utils::valid_handle(target_process_handle) {
        let status = GetLastError();
        println!("{:x}",status as usize);
    }
    
    let device_to_dos = win_utils::build_device_to_dos_map(win_utils::get_drive_mappings());
    
    let mut matched_files:Vec<(SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX,String)> = Vec::new();
    for handle in handles {
        let mut cloned_handle:HANDLE = Default::default();
        let status = DuplicateHandle(target_process_handle,
            handle.HandleValue,
            my_process_handle,
            &mut cloned_handle,
            0,
            0, 
            DUPLICATE_SAME_ACCESS);

        if status == 0 {
            let status = GetLastError();
            println!("DuplicateHandle: {:x}",status as usize);
            continue;
        }

        let mut file_all_info_buf:Vec<u8> = Vec::with_capacity(size_of::<FILE_ALL_INFORMATION>()+512);
        let mut io_status:IO_STATUS_BLOCK = Default::default();
        NtQueryInformationFile(cloned_handle,
            &mut io_status,
            file_all_info_buf.as_mut_ptr() as *mut _,
            file_all_info_buf.capacity() as u32,
            FileAllInformation);
        if io_status.Anonymous.Status != 0 {
            println!("NtQueryInformationFile: {:x}",status as usize);
            CloseHandle(cloned_handle);
            continue;
        }
        let file_all_info = file_all_info_buf.as_ptr().cast::<FILE_ALL_INFORMATION>();
        let name = String::from_utf16(slice::from_raw_parts((*file_all_info).NameInformation.FileName.as_ptr(),(*file_all_info).NameInformation.FileNameLength as usize/2)).unwrap_or_default();
        
        if name.contains(&file_name) {
            let volume_device_path = win_utils::volume_from_handle(cloned_handle);
            let drive_letter = device_to_dos.get(&volume_device_path).cloned().unwrap_or_default();
            let full_path = format!("{}\\{}", drive_letter, name);
            matched_files.push((handle, full_path));
        }
        CloseHandle(cloned_handle);

    }
    matched_files
}

