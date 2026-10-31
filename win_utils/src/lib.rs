use std::{collections::HashMap, ffi::c_void, fs, iter, mem, ptr, slice};

mod test;

use windows_sys::{
    Wdk::{Foundation::{NtClose, NtQueryObject, OBJECT_ATTRIBUTES}, Storage::FileSystem::{FILE_ALL_INFORMATION, FILE_BASIC_INFORMATION, FILE_DIRECTORY_FILE, FILE_DIRECTORY_INFORMATION, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF, FILE_OVERWRITE_IF, FILE_STANDARD_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT, FILE_VOLUME_NAME_INFORMATION, FileAllInformation, FileDirectoryInformation, FileStandardInformation, FileVolumeNameInformation, NtCreateFile, NtOpenProcessToken, NtQueryDirectoryFile, NtQueryInformationFile, NtQueryInformationToken, NtReadFile, NtWriteFile, RtlInitUnicodeStringEx}, System::{SystemInformation::NtQuerySystemInformation, Threading::{NtOpenProcess, NtQueryInformationProcess}}}, Win32::{
        Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, LocalFree, MAX_PATH, NTSTATUS, OBJ_CASE_INSENSITIVE, STATUS_INFO_LENGTH_MISMATCH, STATUS_NO_MORE_FILES, STATUS_PENDING, STATUS_SUCCESS, UNICODE_STRING}, Security::{LookupAccountSidW, PSID, SID_AND_ATTRIBUTES, SID_NAME_USE, SidTypeUnknown, TokenUser}, Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileSize, OPEN_EXISTING, ReadFile, SYNCHRONIZE, SetFilePointer}, System::{Diagnostics::Debug::{FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW, ReadProcessMemory}, IO::IO_STATUS_BLOCK, Kernel::LIST_ENTRY, LibraryLoader::{GetModuleFileNameW, GetModuleHandleW, GetProcAddress, LoadLibraryW}, Memory::{MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE, VirtualAlloc}, ProcessStatus::{EnumProcesses, GetModuleFileNameExW}, Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcess, PEB, PROCESS_BASIC_INFORMATION, PROCESS_DUP_HANDLE, PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE, PROCESS_VM_READ, TerminateProcess}, WindowsProgramming::{CLIENT_ID, RtlInitUnicodeString}}, UI::Shell::{SHCreateDirectoryExW, SHGetSpecialFolderPathW}
    }, core::*
};


#[repr(C)]
struct SYSTEM_HANDLE_INFORMATION_EX {
    NumberOfHandles: usize,
    Reserved:usize,
    Handles: [SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX;1],
}


#[cfg(debug_assertions)]
pub fn print_last_error(error_context:Option<String>) {
    unsafe {
        use std::slice;

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
    match error_context {
        Some(error_context) => {println!("{}: Error: 0x{:x}: {}",error_context,dw,out)},
        None => {println!("Error: 0x{:x}: {}",dw,out)},
    }
    
    LocalFree(output_addr as _);
    }
}


pub fn PCWSTR_to_string(p:*const u16) -> String {
    unsafe {
        if !p.is_null() {
            let mut len = 0;
            let mut ptr = p;
            while *ptr != 0 {
                len += 1;
                ptr = ptr.add(1);
            }
            return String::from_utf16(slice::from_raw_parts(p, len)).unwrap_or_default();
        }
    }
    String::new()
}

#[repr(C)]
struct PEB_LDR_DATA {
    Length:u32,
    Initialized:BOOL,
    SsHandle:HANDLE,
    InLoadOrderModuleList:LIST_ENTRY,
    InMemoryOrderModuleList:LIST_ENTRY,
    InInitializationOrderModuleList:LIST_ENTRY,
    EntryInProgress:usize,
    ShutdownInProgress:BOOL,
    ShutdownThreadId:HANDLE
}

#[repr(C)]
struct LDR_DATA_TABLE_ENTRY {
    InLoadOrderLinks:LIST_ENTRY,
    InMemoryOrderLinks:LIST_ENTRY,
    InInitializationOrderLinks:LIST_ENTRY,
    DllBase:usize,
    EntryPoint:usize,
    SizeOfImage:u32,
    FullDllName:UNICODE_STRING,
    BaseDllName:UNICODE_STRING,
    /*idc about the rest rn */
}

struct my_module_info {
    DllBase:usize,
    EntryPoint:usize,
    SizeOfImage:u32,
    FullDllName:String,
    BaseDllName:String
}

pub fn get_module_info(pid:u32,module_name:&str) -> Option<my_module_info> {
    unsafe {
    let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_VM_READ,0,pid);

    let mut basic_info:PROCESS_BASIC_INFORMATION = std::mem::zeroed();
    let mut length:u32 = 0;
    NtQueryInformationProcess(process_handle,0,&mut basic_info as *mut _ as *mut _,size_of::<PROCESS_BASIC_INFORMATION>() as u32,&mut length);

    let mut peb: Vec<u8> = Vec::with_capacity(size_of::<PEB>());//std::mem::zeroed();
    let mut length:usize = 0;
    if pid == GetCurrentProcessId() {
        std::ptr::copy(basic_info.PebBaseAddress as *const _,peb.as_mut_ptr(),size_of::<PEB>());
    } else {
        ReadProcessMemory(process_handle,basic_info.PebBaseAddress as *const _, peb.as_mut_ptr() as *mut _, size_of::<PEB>(),&mut length);
    }

    let ldr_data = peb.as_ptr().add(0x18).cast::<PEB_LDR_DATA>();
    let start = (*ldr_data).InLoadOrderModuleList;
    let mut current:LIST_ENTRY = start; 
    loop {
        if current.Flink == start.Flink {
            break
        }
        let mut entry:Vec<u8> = Vec::with_capacity(size_of::<LDR_DATA_TABLE_ENTRY>());
        std::ptr::copy(current.Flink,entry.as_mut_ptr() as _,size_of::<LDR_DATA_TABLE_ENTRY>());
        entry.set_len(size_of::<LDR_DATA_TABLE_ENTRY>());

        let test = entry.as_ptr().cast::<LDR_DATA_TABLE_ENTRY>();

        let full_module_path = String::from_utf16(slice::from_raw_parts((*test).FullDllName.Buffer,(*test).FullDllName.Length as usize)).unwrap();
        let base_module_name = String::from_utf16(slice::from_raw_parts((*test).BaseDllName.Buffer,(*test).BaseDllName.Length as usize)).unwrap();
        if full_module_path.to_lowercase().trim().ends_with(module_name) {
            return Some(my_module_info{
                DllBase:(*test).DllBase,
                EntryPoint:(*test).EntryPoint,
                SizeOfImage:(*test).SizeOfImage,
                FullDllName:full_module_path,
                BaseDllName:base_module_name,
            });
        }
        current = (*test).InLoadOrderLinks;
    }
    return None
    }
}

/// Converts a device path (e.g., \Device\HarddiskVolume3\Users\...) to a DOS path (e.g., C:\Users\...)
/// Returns None if no matching drive mapping is found
pub fn device_path_to_dos_path(device_path: &str, mappings: &[(String, String)]) -> Option<String> {
    for (dos_prefix, device_prefix) in mappings {
        if device_path.starts_with(device_prefix) {
            let remainder = &device_path[device_prefix.len()..];
            return Some(format!(r#"\??\{}{}"#, dos_prefix, remainder));
        }
    }
    None
}

const FilePositionInformation: i32 = 14;

type FnNtOpenSymbolicLinkObject = unsafe extern "system" fn(
    LinkHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *const OBJECT_ATTRIBUTES,
) -> NTSTATUS;

type FnNtQuerySymbolicLinkObject = unsafe extern "system" fn(
    LinkHandle: HANDLE,
    LinkTarget: *mut UNICODE_STRING,
    ReturnedLength: *mut u32,
) -> NTSTATUS;


/// Gets the drive letter to device path mappings using NtOpenSymbolicLinkObject and NtQuerySymbolicLinkObject
pub unsafe fn get_drive_mappings() -> Vec<(String, String)> {
    let ntdll = GetModuleHandleW(w!("ntdll.dll"));
    if ntdll.is_null() {
        return Vec::new();
    }

    let NtOpenSymbolicLinkObject: FnNtOpenSymbolicLinkObject = 
        std::mem::transmute(GetProcAddress(ntdll, s!("NtOpenSymbolicLinkObject")).unwrap());
    let NtQuerySymbolicLinkObject: FnNtQuerySymbolicLinkObject = 
        std::mem::transmute(GetProcAddress(ntdll, s!("NtQuerySymbolicLinkObject")).unwrap());

    let mut mappings: Vec<(String, String)> = Vec::new();

    // Iterate through drive letters A-Z
    for drive_letter in b'A'..=b'Z' {
        let symlink_path = format!("\\??\\{}:", drive_letter as char);
        let symlink_wide: Vec<u16> = symlink_path.encode_utf16().chain(iter::once(0)).collect();

        let mut unicode_str: UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut unicode_str, symlink_wide.as_ptr());

        let obj_attr: OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &unicode_str,
            Attributes: 0x40, // OBJ_CASE_INSENSITIVE
            SecurityDescriptor: std::ptr::null_mut(),
            SecurityQualityOfService: std::ptr::null_mut(),
        };

        let mut link_handle: HANDLE = std::ptr::null_mut();
        
        // SYMBOLIC_LINK_QUERY = 0x0001
        let status = NtOpenSymbolicLinkObject(&mut link_handle, 0x0001, &obj_attr);
        if status != STATUS_SUCCESS {
            continue; // Drive letter not mapped
        }

        // Query the target of the symbolic link
        let mut target_buffer: Vec<u16> = vec![0u16; 512];
        let mut target_unicode: UNICODE_STRING = UNICODE_STRING {
            Length: 0,
            MaximumLength: (target_buffer.len() * 2) as u16,
            Buffer: target_buffer.as_mut_ptr(),
        };
        let mut returned_length: u32 = 0;

        let status = NtQuerySymbolicLinkObject(link_handle, &mut target_unicode, &mut returned_length);
        CloseHandle(link_handle);

        if status == STATUS_SUCCESS {
            let device_path = String::from_utf16_lossy(
                &target_buffer[..(target_unicode.Length as usize / 2)]
            );
            let dos_path = format!("{}:", drive_letter as char);
            
            mappings.push((dos_path, device_path));
        }
    }

    mappings
}

const ObjectTypesInformation: i32 = 3;
const ObjectHandleFlagInformation: i32 = 4;
#[repr(C)]
struct OBJECT_TYPES_INFORMATION {
    NumberOfTypes: u32,
}

#[repr(C)]
struct OBJECT_TYPE_INFORMATION {
    TypeName: UNICODE_STRING,
    TotalNumberOfObjects: u32,
    TotalNumberOfHandles: u32,
    TotalPagedPoolUsage: u32,
    TotalNonPagedPoolUsage: u32,
    TotalNamePoolUsage: u32,
    TotalHandleTableUsage: u32,
    HighWaterNumberOfObjects: u32,
    HighWaterNumberOfHandles: u32,
    HighWaterPagedPoolUsage: u32,
    HighWaterNonPagedPoolUsage: u32,
    HighWaterNamePoolUsage: u32,
    HighWaterHandleTableUsage: u32,
    InvalidAttributes: u32,
    GenericMapping: [u32; 4], // GENERIC_MAPPING
    ValidAccessMask: u32,
    SecurityRequired: u8,
    MaintainHandleCount: u8,
    TypeIndex: u8,
    ReservedByte: u8,
    PoolType: u32,
    DefaultPagedPoolCharge: u32,
    DefaultNonPagedPoolCharge: u32,
}

const SystemExtendedHandleInformation: i32 = 64;


#[repr(C)]
#[derive(Clone, Copy)]
pub struct SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX {
    pub Object: usize,
    pub UniqueProcessId: HANDLE,
    pub HandleValue: HANDLE,
    pub GrantedAccess: u32,
    pub CreatorBackTraceIndex: u16,
    pub ObjectTypeIndex: u16,
    pub HandleAttributes: u32,
    pub Reserved: u32,
}


pub unsafe fn get_current_process_file_handles(file_type_index: u16) -> Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX> {
    let current_pid = GetCurrentProcessId() as usize;
    let mut buf: Vec<u8> = Vec::new();
    
    loop {
        let mut rlength: u32 = 0;
        let status = NtQuerySystemInformation(
            SystemExtendedHandleInformation,
            buf.as_mut_ptr() as *mut _,
            buf.capacity() as u32,
            &mut rlength,
        );
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf = Vec::with_capacity(rlength as usize + 4096);
        } else {
            buf.set_len(rlength as usize);
            break;
        }
    }

    let system_handle_info = buf.as_ptr().cast::<SYSTEM_HANDLE_INFORMATION_EX>();
    let mut matched_handles: Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX> = Vec::new();
    
    for index in 0..(*system_handle_info).NumberOfHandles {
        let handle_info = (*system_handle_info).Handles.as_ptr().add(index);
        if (*handle_info).UniqueProcessId as usize == current_pid
            && (*handle_info).ObjectTypeIndex == file_type_index
        {
            matched_handles.push(*handle_info);
        }
    }
    matched_handles
}

pub unsafe fn get_process_file_handles(
    target_pid: u32, 
    file_type_index: u16
) -> Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX> {
    let mut buf: Vec<u8> = Vec::new();
    
    loop {
        let mut rlength: u32 = 0;
        let status = NtQuerySystemInformation(
            SystemExtendedHandleInformation,
            buf.as_mut_ptr() as *mut _,
            buf.capacity() as u32,
            &mut rlength,
        );
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf = Vec::with_capacity(rlength as usize + 4096);
        } else {
            buf.set_len(rlength as usize);
            break;
        }
    }

    let system_handle_info = buf.as_ptr().cast::<SYSTEM_HANDLE_INFORMATION_EX>();
    let mut matched_handles: Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX> = Vec::new();
    
    for index in 0..(*system_handle_info).NumberOfHandles {
        let handle_info = (*system_handle_info).Handles.as_ptr().add(index);
        if (*handle_info).UniqueProcessId as u32 == target_pid
            && (*handle_info).ObjectTypeIndex == file_type_index
        {
            matched_handles.push(*handle_info);
        }
    }
    matched_handles
}

pub unsafe fn get_file_path_from_handle(handle: HANDLE) -> Option<String> {
    let handle_copy = handle as usize;
    let query_result = std::thread::spawn(move || -> Option<String> {
        unsafe {
            let mut file_all_info_buf: Vec<u8> = vec![0u8; size_of::<FILE_ALL_INFORMATION>() + 512];
            let mut io_status: IO_STATUS_BLOCK = std::mem::zeroed();
            
            let status = NtQueryInformationFile(
                handle_copy as HANDLE,
                &mut io_status,
                file_all_info_buf.as_mut_ptr() as *mut _,
                file_all_info_buf.len() as u32,
                FileAllInformation,
            );
            
            const STATUS_BUFFER_OVERFLOW: i32 = -2147483643i32;
            if status != 0 && status != STATUS_BUFFER_OVERFLOW {
                return None;
            }
            if io_status.Anonymous.Status != 0 && io_status.Anonymous.Status != STATUS_BUFFER_OVERFLOW {
                return None;
            }
            
            let file_all_info = file_all_info_buf.as_ptr().cast::<FILE_ALL_INFORMATION>();
            let name_len = (*file_all_info).NameInformation.FileNameLength as usize / 2;
            if name_len == 0 || name_len > 32767 {
                return None;
            }
            
            let name = String::from_utf16(slice::from_raw_parts(
                (*file_all_info).NameInformation.FileName.as_ptr(),
                name_len,
            ))
            .ok()?;
            
            Some(name)
        }
    });

    let timeout = std::time::Duration::from_millis(200);
    let start = std::time::Instant::now();
    loop {
        if query_result.is_finished() {
            return match query_result.join() {
                Ok(result) => result,
                Err(_) => None,
            };
        }
        if start.elapsed() > timeout {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub unsafe fn get_volume_from_handle(handle: HANDLE) -> String {
    let handle_copy = handle as usize;
    let query_result = std::thread::spawn(move || -> String {
        unsafe {
            let mut buf: Vec<u8> = vec![0u8; size_of::<FILE_VOLUME_NAME_INFORMATION>() + 256];
            let mut io_status: IO_STATUS_BLOCK = std::mem::zeroed();
            
            let status = NtQueryInformationFile(
                handle_copy as HANDLE,
                &mut io_status,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                FileVolumeNameInformation,
            );
            
            if status != 0 {
                return String::new();
            }
            if io_status.Anonymous.Status != 0 {
                return String::new();
            }
            
            let volume_name = buf.as_ptr().cast::<FILE_VOLUME_NAME_INFORMATION>();
            let name_len = (*volume_name).DeviceNameLength as usize / 2;
            if name_len == 0 || name_len > 32767 {
                return String::new();
            }
            
            String::from_utf16(slice::from_raw_parts(
                (*volume_name).DeviceName.as_ptr(),
                name_len,
            ))
            .unwrap_or_default()
        }
    });

    let timeout = std::time::Duration::from_millis(200);
    let start = std::time::Instant::now();
    loop {
        if query_result.is_finished() {
            return match query_result.join() {
                Ok(result) => result,
                Err(_) => String::new(),
            };
        }
        if start.elapsed() > timeout {
            return String::new();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub fn has_read_access(granted_access: u32) -> bool {
    const FILE_READ_DATA: u32 = 0x0001;
    (granted_access & FILE_READ_DATA) != 0 || (granted_access & GENERIC_READ as u32) != 0
}

pub unsafe fn get_other_module_pids() -> Vec<u32> {

    let mut my_module_file_path:Vec<u16> = Vec::with_capacity(512);
    let written = GetModuleFileNameW(std::ptr::null_mut(),my_module_file_path.as_mut_ptr(),512);
    my_module_file_path.set_len(written as usize);
    let my_module_file_path = String::from_utf16(slice::from_raw_parts(my_module_file_path.as_mut_ptr(), written as usize)).unwrap();

    type EnumProcessesFn = unsafe extern "system" fn(
        lpidProcess: *mut u32,
        cb: u32,
        lpcbNeeded: *mut u32,
    ) -> i32;
    
    let psapi = LoadLibraryW(w!("psapi.dll"));
    if psapi.is_null() {
        return Vec::new();
    }
    
    let enum_processes: EnumProcessesFn = std::mem::transmute(
        GetProcAddress(psapi, s!("EnumProcesses"))
    );
    
    let current_pid = GetCurrentProcessId();
    let mut pids: Vec<u32> = vec![0u32; 2048];
    let mut bytes_returned: u32 = 0;
    
    let result = enum_processes(
        pids.as_mut_ptr(),
        (pids.len() * 4) as u32,
        &mut bytes_returned,
    );
    
    if result == 0 {
        return Vec::new();
    }
    
    let num_pids = bytes_returned as usize / 4;
    let mut msedge_pids: Vec<u32> = Vec::new();
    
    for i in 0..num_pids {
        let pid = pids[i];
        if pid == 0 || pid == current_pid {
            continue;
        }
        
        let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if process_handle.is_null() {
            continue;
        }
        
        let mut module_name:Vec<u16> = Vec::with_capacity(512);
        let len = GetModuleFileNameExW(process_handle, std::ptr::null_mut(), module_name.as_mut_ptr(), 512);
        module_name.set_len(len as usize);
        CloseHandle(process_handle);
        
        if len > 0 {
            let name = String::from_utf16(slice::from_raw_parts(module_name.as_mut_ptr(), len as usize)).unwrap();
            if name.eq_ignore_ascii_case(&my_module_file_path) {
                msedge_pids.push(pid);
            }
        }
    }
    
    msedge_pids
}


pub unsafe fn copy_file_from_handle(handle: HANDLE, dest_path: &str) -> Result<(), String> {
    let file_size = GetFileSize(handle, std::ptr::null_mut());
    if file_size == u32::MAX {
        return Err("Failed to get file size".to_string());
    }

    SetFilePointer(handle, 0, std::ptr::null_mut(), FILE_BEGIN);

    let mut buffer: Vec<u8> = vec![0u8; file_size as usize];
    let mut bytes_read: u32 = 0;
    
    let read_result = ReadFile(
        handle,
        buffer.as_mut_ptr() as *mut _,
        file_size,
        &mut bytes_read,
        std::ptr::null_mut(),
    );

    if read_result == 0 {
        return Err(format!("ReadFile failed: {:x}", GetLastError()));
    }

    buffer.truncate(bytes_read as usize);

    fs::write(dest_path, &buffer).map_err(|e| e.to_string())?;

    Ok(())
}

pub unsafe fn duplicate_handle_from_process(
    source_pid: u32,
    source_handle: HANDLE,
) -> Option<HANDLE> {
    let source_process = OpenProcess(PROCESS_DUP_HANDLE, 0, source_pid);
    if source_process.is_null() {
        return None;
    }
    
    let mut duplicated_handle: HANDLE = std::ptr::null_mut();
    let result = DuplicateHandle(
        source_process,
        source_handle,
        GetCurrentProcess(),
        &mut duplicated_handle,
        0,
        0,
        DUPLICATE_SAME_ACCESS,
    );
    
    CloseHandle(source_process);
    
    if result != 0 && !duplicated_handle.is_null() {
        Some(duplicated_handle)
    } else {
        None
    }
}

pub unsafe fn file_information(path:String) -> Result<FILE_BASIC_INFORMATION,i32> {
    let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut unicode_string: UNICODE_STRING = std::mem::zeroed();
    RtlInitUnicodeString(&mut unicode_string, path_wide.as_ptr());

    let object_attributes = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: ptr::null_mut(),
            ObjectName: &mut unicode_string,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
    };
        
    let mut file_info: FILE_BASIC_INFORMATION = std::mem::zeroed();

    let ntdll = LoadLibraryW(w!("ntdll.dll"));

    let NtQueryAttributesFile:extern "system" fn (ObjectAttributes:&OBJECT_ATTRIBUTES,FileInformation:&mut FILE_BASIC_INFORMATION) -> i32 = std::mem::transmute(GetProcAddress(ntdll, s!("NtQueryAttributesFile")));

    let status = NtQueryAttributesFile(&object_attributes, &mut file_info);
    if status != windows_sys::Win32::Foundation::STATUS_SUCCESS {
        return Err(status);
    };
    Ok(file_info)
}

pub unsafe fn user() -> Result<String,i32> {
    let mut process_handle:HANDLE = Default::default();
    let mut obj_attr:OBJECT_ATTRIBUTES = std::mem::zeroed();
    let mut cid:CLIENT_ID = std::mem::zeroed();
    cid.UniqueProcess = GetCurrentProcessId() as HANDLE;

    let status = NtOpenProcess(&mut process_handle,0x2000000,&mut obj_attr,&mut cid);
    if status != windows_sys::Win32::Foundation::STATUS_SUCCESS {
        return Err(status)
    }

    let mut user_handle:HANDLE = Default::default();
    let status = NtOpenProcessToken(process_handle,0x2000000,&mut user_handle);
    if status != windows_sys::Win32::Foundation::STATUS_SUCCESS {
        return Err(status)
    }

    let mut buf_len:u32 = 0;
    let status = NtQueryInformationToken(user_handle,TokenUser,std::ptr::null_mut(),0,&mut buf_len);
    if status != windows_sys::Win32::Foundation::STATUS_BUFFER_TOO_SMALL {
        return Err(status)
    }

    let mut sid_buf: Vec<u8> = vec![0u8; buf_len as usize];
    let status = NtQueryInformationToken(user_handle,TokenUser,sid_buf.as_mut_ptr() as _,buf_len,&mut buf_len);
    if status != windows_sys::Win32::Foundation::STATUS_SUCCESS {
        return Err(status)
    }

    let token_user = sid_buf.as_ptr().cast::<SID_AND_ATTRIBUTES>();
    let sid = (*token_user).Sid;

    match sid_to_string(sid) {
        Ok((user,_)) => return Ok(user),
        Err(status) => return Err(status)
    }
}


fn sid_to_string(psid:PSID) -> Result<(String,String),i32> {
    unsafe {
        let mut name_len = 0;
        let mut domain_len = 0;
        let mut name_use: SID_NAME_USE = SidTypeUnknown;

        LookupAccountSidW(std::ptr::null_mut(),
        psid,
        std::ptr::null_mut(),
        &mut name_len,
        std::ptr::null_mut(),
        &mut domain_len,
        &mut name_use);

        let mut name:Vec<u16> = vec![0; name_len as usize];
        let mut domain:Vec<u16> = vec![0; domain_len as usize];

        let status = LookupAccountSidW(std::ptr::null_mut(),
        psid,
        name.as_mut_ptr(),
        &mut name_len,
        domain.as_mut_ptr(),
        &mut domain_len,
        &mut name_use);
        if status == 0 {
            return Err(status);
        }
        return Ok((String::from_utf16_lossy(&name[..name_len as usize]),String::from_utf16_lossy(&domain[..domain_len as usize])))
    }
}

pub unsafe fn get_object_index(type_name:String) -> u8 {
    let mut buf:Vec<u8> = Vec::new();
    loop {
        let mut rlength:u32 = 0;
        let status = NtQueryObject(std::ptr::null_mut(),
            ObjectTypesInformation,
            buf.as_mut_ptr() as *mut _,
            buf.capacity() as u32,
            &mut rlength);
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf = Vec::with_capacity(rlength as usize + 4096);
        } else {
            buf.set_len(rlength as usize);
            break
        }
    }

    let object_type_info = buf.as_ptr().cast::<OBJECT_TYPES_INFORMATION>();
    let mut offset:usize = 8;
    for i in 1..(*object_type_info).NumberOfTypes {
        let type_info = buf.as_ptr().byte_add(offset).cast::<OBJECT_TYPE_INFORMATION>();
        let name = String::from_utf16(slice::from_raw_parts((*type_info).TypeName.Buffer,(*type_info).TypeName.Length as usize/2)).unwrap();
        if name.to_lowercase().trim() == type_name.to_lowercase().trim() {
            return (*type_info).TypeIndex;
        } 
        let unaligned_offset = (0x68+(*type_info).TypeName.MaximumLength-1) as usize + offset;
        let diff = 8 - (unaligned_offset%8);
        offset = unaligned_offset +diff;
    }
    0
}

pub unsafe fn processes(module:String) -> Vec<u32> {
    let mut pids: Vec<u32> = vec![0u32; 2048];
    let mut bytes_returned: u32 = 0;
    
    let result = EnumProcesses(
        pids.as_mut_ptr(),
        (pids.len() * 4) as u32,
        &mut bytes_returned,
    );
    
    if result == 0 {
        return Vec::new();
    }
    
    let num_pids = bytes_returned as usize / 4;
    let mut msedge_pids: Vec<u32> = Vec::new();
    
    for i in 0..num_pids {
        let pid = pids[i];
        if pid == 0  {
            continue;
        }
        
        // Open process to check if it's msedge.exe
        let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if process_handle.is_null() {
            continue;
        }
        
        let mut module_name: [u16; 260] = [0; 260];
        let len = GetModuleFileNameExW(process_handle, std::ptr::null_mut(), module_name.as_mut_ptr(), 260);
        CloseHandle(process_handle);
        
        if len > 0 {
            let name = String::from_utf16_lossy(&module_name[..len as usize]);
            if name.to_lowercase().contains(&module) {
                msedge_pids.push(pid);
            }
        }
    }
    
    msedge_pids
}

 pub unsafe fn valid_handle(handle:HANDLE) -> bool {
    if handle as usize == 0xffffffffffffffff  {
        return false;
    } else if handle as usize == 0 {
        return false;
    }
    true
 }

 #[derive(Clone)]
 pub struct FILE_INFO {
    pub EndOfFile:i64,
    pub FileAttributes:u32,
    pub FileName:String,
}

 pub unsafe fn list_directory(path:String) -> Result<Vec<FILE_INFO>,u32> {
    let mut path:Vec<u16> = path.encode_utf16().collect();
    if !path.ends_with(&[0]) {
        path.push(0);
    }
    let dir_handle = CreateFileW(path.as_ptr(),
        GENERIC_READ,
        FILE_SHARE_READ,
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        std::ptr::null_mut());
    if !valid_handle(dir_handle) {
        let status = GetLastError();
        return Err(status);
    }

    let mut list:Vec<FILE_INFO> = Vec::new();
    loop {
        let buf_len = size_of::<FILE_DIRECTORY_INFORMATION>()+(256*2);
        let mut buf:Vec<u8> = Vec::with_capacity(buf_len);
        let mut iostatus:IO_STATUS_BLOCK = Default::default();
        let status = NtQueryDirectoryFile(dir_handle,
            std::ptr::null_mut(),
            None,
            std::ptr::null_mut(),
            &mut iostatus,
            buf.as_mut_ptr() as *mut _,
            buf_len as u32,
            FileDirectoryInformation,
            true,
            std::ptr::null_mut(),
            false);
        if status != 0 {
            break;
        }
        let file_info = buf.as_ptr().cast::<FILE_DIRECTORY_INFORMATION>();
        let FileName = String::from_utf16(slice::from_raw_parts((*file_info).FileName.as_ptr(), (*file_info).FileNameLength as usize/2)).unwrap();
        let EndOfFile =(*file_info).EndOfFile;
        let FileAttributes = (*file_info).FileAttributes;
        list.push(FILE_INFO { EndOfFile, FileAttributes, FileName });
    }
    CloseHandle(dir_handle);
    Ok(list)
 }


 pub unsafe fn open_handles(pid:Option<usize>,objecttype:Option<u16>) -> Result<Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX>,u32> {
    let mut buf:Vec<u8> = Vec::new();
    loop {
        let mut rlength:u32 = 0;
        let status = NtQuerySystemInformation(SystemExtendedHandleInformation,
            buf.as_mut_ptr() as *mut _,
            buf.capacity() as u32,
            &mut rlength);
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf = Vec::with_capacity(rlength as usize + 4096);
        } else {
            buf.set_len(rlength as usize);
            break
        }
    }

    let system_handle_info = buf.as_ptr().cast::<SYSTEM_HANDLE_INFORMATION_EX>();
    let mut matched_handles: Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX> = Vec::new();
    for index in 0..(*system_handle_info).NumberOfHandles {
        let handle_info = (*system_handle_info).Handles.as_ptr().add(index);
        if  (pid == None || pid.is_some_and(|v| (*handle_info).UniqueProcessId as usize == v) ) && (objecttype == None || objecttype.is_some_and(|v| (*handle_info).ObjectTypeIndex == v )) {
            matched_handles.push(*handle_info);

        }
    }
    Ok(matched_handles)
 }

 pub fn build_device_to_dos_map(dos_to_drive:Vec<(String,String)>) -> std::collections::HashMap<String,String> {
    let mut device_to_drive: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for x in dos_to_drive {
        device_to_drive.insert(x.1, x.0);
    };
    device_to_drive
}

pub fn build_dos_to_device_map(dos_to_drive:Vec<(String,String)>) -> std::collections::HashMap<String,String> {
    let mut device_to_drive: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for x in dos_to_drive {
        device_to_drive.insert(x.0, x.1);
    };
    device_to_drive
}

pub unsafe fn volume_from_handle(handle:HANDLE) -> String {
    //NtQueryInformationFile+FileVolumeNameInformation
    //"\\Device\\HarddiskVolume3"
    let mut buf:Vec<u8> = Vec::with_capacity(size_of::<FILE_VOLUME_NAME_INFORMATION>()+256);
    let mut io_status:IO_STATUS_BLOCK = Default::default();
    NtQueryInformationFile(handle,
        &mut io_status,
        buf.as_mut_ptr() as *mut _,
        buf.capacity() as u32,
    FileVolumeNameInformation);
    if io_status.Anonymous.Status != 0 {
        println!("{:x}",io_status.Anonymous.Status);
        return "".to_string();
    }
    
    let volume_name = buf.as_ptr().cast::<FILE_VOLUME_NAME_INFORMATION>();
    let volume_name = String::from_utf16(slice::from_raw_parts((*volume_name).DeviceName.as_ptr(), (*volume_name).DeviceNameLength as usize/2)).unwrap();
        
    volume_name
}


#[repr(C)]
struct CURDIR {
    DosPath:UNICODE_STRING,
    Handle:HANDLE,
}

#[repr(C)]
struct RTL_USER_PROCESS_PARAMETERS {
    MaximumLength:u32,
    Length:u32,
    Flags:u32,
    DebugFlags:u32,
    ConsoleHandle:HANDLE,
    ConsoleFlags:u32,
    StandardInput:HANDLE,
    StandardOutput:HANDLE,
    StandardError:HANDLE,
    CurrentDirectory:CURDIR,
    DllPath:UNICODE_STRING,
    ImagePathName:UNICODE_STRING,
    CommandLine:UNICODE_STRING,
    Environment:usize,
    StartingX:u32,
    StartingY:u32,
    CountX:u32,
    CountY:u32,
    CountCharsX:u32,
    CountCharsY:u32,
    FillAttributes:u32,

    WindowFlags:u32,
    ShowWindowFlags:u32,
    WindowTitle:UNICODE_STRING,
    DesktopInfo:UNICODE_STRING,
    ShellInfo:UNICODE_STRING,
    RunTimeData:UNICODE_STRING,
    currentDirectories:[u8;0x300],
    EnvironmentSize:usize,
    EnvironmentVersion:usize,
    /*idc about the rest */
}

pub unsafe fn unicode_peb_unicode(pid:u32) -> HashMap<String,String> {
    let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_VM_READ,0,pid);

    let mut basic_info:PROCESS_BASIC_INFORMATION = std::mem::zeroed();
    let mut length:u32 = 0;
    NtQueryInformationProcess(process_handle,0,&mut basic_info as *mut _ as *mut _,size_of::<PROCESS_BASIC_INFORMATION>() as u32,&mut length);

    let mut peb: PEB = std::mem::zeroed();
    let mut length:usize = 0;
    ReadProcessMemory(process_handle,basic_info.PebBaseAddress as *const _, &mut peb as *mut _ as *mut _, size_of::<PEB>(),&mut length);
    
    let mut process_params: RTL_USER_PROCESS_PARAMETERS = std::mem::zeroed();
    let mut length:usize = 0;
    ReadProcessMemory(process_handle,peb.ProcessParameters as *const _,&mut process_params as *mut _ as *mut _,size_of::<RTL_USER_PROCESS_PARAMETERS>(),&mut length);


    let mut env_buf_size:usize = process_params.EnvironmentSize;
    let mut env_buf:Vec<u16> = Vec::with_capacity(env_buf_size as usize);
    ReadProcessMemory(process_handle, process_params.Environment as *const _, env_buf.as_mut_ptr() as *mut _, env_buf_size, &mut env_buf_size);
    if env_buf_size == 0 {
        println!("{:x}", GetLastError());
    }
    env_buf.set_len(env_buf_size);
    
    let mut current_kv:Vec<u16> = Vec::new();
    let mut env_vars: HashMap<String, String> = HashMap::new();
    for i in 0..env_buf.len() {
        if i == env_buf.len()-1 {
            break
        }
        if env_buf[i] == 0 && env_buf[i+1] == 0 {
            let kv = String::from_utf16(&current_kv).unwrap_or_default();
            let kv: Vec<&str> = kv.splitn(2, '=').collect();
            if kv.len() == 2 {
                env_vars.insert(String::from(kv[0]), String::from(kv[1]));
            }
            current_kv.clear();
            break;
        } else if env_buf[i] == 0 {
            let kv = String::from_utf16(&current_kv).unwrap_or_default();
            let kv: Vec<&str> = kv.splitn(2, '=').collect();
            if kv.len() == 2 {
                env_vars.insert(String::from(kv[0]), String::from(kv[1]));
            }
            current_kv.clear();
        } else {
            current_kv.push(env_buf[i]);
        }
    }
    env_vars
}

type FnNtQueryAttributesFile = unsafe extern "system" fn(
    ObjectAttributes:*mut OBJECT_ATTRIBUTES,
    FileInformation:*mut FILE_BASIC_INFORMATION,
) -> NTSTATUS;

pub fn copy_dir_resursive_native(src: &str,dst:&str,exclude:Vec<&str>) -> i32 {

    let src_native = to_unc_utf16(src);
    let dst_native = to_unc_utf16(dst);

    for p in exclude.clone() {
        if src.contains(p) {
            return 0;
        }
    }

    #[cfg(debug_assertions)]
    {
        println!("[copying dir] {} -> {}",src,dst);
    }

    unsafe {
        let ntdll = GetModuleHandleW(w!("ntdll.dll"));
        if ntdll.is_null() {
            return GetLastError() as i32;
        }

        let NtQueryAttributesFile:FnNtQueryAttributesFile = std::mem::transmute(GetProcAddress(ntdll, s!("NtQueryAttributesFile")).unwrap());

        let mut source_path_unicode:UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut source_path_unicode,
            src_native.as_ptr());

        let mut src_obj_attr:OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES { Length: size_of::<OBJECT_ATTRIBUTES>() as u32, 
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &mut source_path_unicode,
            Attributes: 0,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService:std::ptr::null()};


        let mut src_info:FILE_BASIC_INFORMATION = mem::zeroed();
        let status = NtQueryAttributesFile(&mut src_obj_attr,&mut src_info);

        if status != STATUS_SUCCESS {
            return status;
        }

        let source_create_options = if src_info.FileAttributes&0x10 != 0 {
            FILE_DIRECTORY_FILE
        } else {
            FILE_NON_DIRECTORY_FILE 
        };

        let mut status:IO_STATUS_BLOCK = Default::default();
        let mut source_handle:HANDLE = Default::default();
        NtCreateFile(&mut source_handle,
            GENERIC_READ|GENERIC_WRITE|FILE_LIST_DIRECTORY|SYNCHRONIZE,
            &mut src_obj_attr,
            &mut status,
            &0,
            0,
            FILE_SHARE_READ|FILE_SHARE_WRITE,
            FILE_OPEN,
            source_create_options|FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0);

        if source_handle as usize == 0 || source_handle as usize == 0xffffffffffffffff {
            return GetLastError() as i32;
        }

        //open dest

        let mut destination_path_unicode:UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut destination_path_unicode, dst_native.as_ptr());

        let mut dst_obj_attr:OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES { Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &mut destination_path_unicode,
            Attributes: 0,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };

        let mut status:IO_STATUS_BLOCK = Default::default();
        let mut destination_handle:HANDLE = Default::default();
        NtCreateFile(&mut destination_handle,
            GENERIC_READ|GENERIC_WRITE|SYNCHRONIZE,
            &mut dst_obj_attr,
            &mut status,
            &0,
            0,
            0,
            FILE_OPEN_IF,
            source_create_options|FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0);
    
        if destination_handle as usize == 0 || destination_handle as usize == 0xffffffffffffffff {
            return GetLastError() as i32;
        }

        if src_info.FileAttributes&0x10 != 0 {
            let mut size:u32 = 512;
            let mut repeat:bool = false;
            let mut buffer: Vec<u8>;
            loop {
                buffer = vec![0; size as usize];
                let mut iostatus:IO_STATUS_BLOCK = Default::default();
                let status = NtQueryDirectoryFile(source_handle,
                    std::ptr::null_mut(),
                    None,
                    std::ptr::null(),
                    &mut iostatus,
                    buffer.as_mut_ptr() as *mut std::ffi::c_void,
                    size,
                    FileDirectoryInformation,
                    false,
                    std::ptr::null(),
                    repeat);

                if repeat {
                    break
                }
                if status == STATUS_NO_MORE_FILES {
                    repeat = true;
                    continue;
                }
                size = size * 2;
            }

            if buffer.is_empty() {
                return 0
            }

            let mut offset:usize = 0;
            loop {
                let file_directory_info_entry = buffer.as_ptr().add(offset).cast::<FILE_DIRECTORY_INFORMATION>();

                let filename= String::from_utf16(slice::from_raw_parts((*file_directory_info_entry).FileName.as_ptr(), (*file_directory_info_entry).FileNameLength as usize /2)).unwrap();

                if !(filename == "." || filename == "..") {

                    let dst_processed:String;
                    if dst.ends_with(r"\") {
                        dst_processed = format!("{}{}",dst,filename.as_str());
                    } else {
                        dst_processed = format!("{}\\{}",dst,filename);
                    };

                    let src_processed:String;
                    if src.ends_with(r"\") {
                        src_processed = format!("{}{}",src,filename.as_str());
                    } else {
                        src_processed = format!("{}\\{}",src,filename);
                    };

                if (*file_directory_info_entry).FileAttributes&0x10 != 0 {
                    copy_dir_resursive_native(src_processed.as_str(),dst_processed.as_str(),exclude.clone());
                } else {
                    let status = copy_file_native(src_processed.as_str(),dst_processed.as_str());
                    if status != STATUS_SUCCESS {
                        return status;
                    }
                }
                }

                if (*file_directory_info_entry).NextEntryOffset == 0 {
                    break
                }
                offset += (*file_directory_info_entry).NextEntryOffset as usize;
            }

        } else {
            return copy_file_native(src, dst)
        }
        return 0;
    }
}

pub fn copy_file_native(src:&str,dst: &str) -> i32 {
    #[cfg(debug_assertions)]
    {
        println!("[copying file] {} -> {}",src,dst);
    }
    unsafe {
        let source_path:Vec<u16> = src.encode_utf16().chain(iter::once(0)).collect();
        let mut source_path_unicode:UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut source_path_unicode, source_path.as_ptr());

        let mut src_obj_attr:OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES { 
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32, 
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &mut source_path_unicode,
            Attributes: 0,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null()
        };

        let mut status:IO_STATUS_BLOCK = Default::default();
        let mut source_handle:HANDLE = Default::default();
        
        let nt_status = NtCreateFile(
            &mut source_handle,
            GENERIC_READ,
            &mut src_obj_attr,
            &mut status,
            std::ptr::null(),
            0,
            FILE_SHARE_READ,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
            std::ptr::null(),
            0
        );
        
        if nt_status != STATUS_SUCCESS {
            return nt_status;
        }

        let mut file_info: FILE_STANDARD_INFORMATION = mem::zeroed();
        let mut info_status: IO_STATUS_BLOCK = Default::default();
        
        let nt_status = NtQueryInformationFile(
            source_handle,
            &mut info_status,
            &mut file_info as *mut _ as *mut c_void,
            size_of::<FILE_STANDARD_INFORMATION>() as u32,
            FileStandardInformation
        );
        
        if nt_status != STATUS_SUCCESS {
            NtClose(source_handle);
            return nt_status;
        }

        let file_size = file_info.EndOfFile as usize;

        let dest_path:Vec<u16> = dst.encode_utf16().chain(iter::once(0)).collect();
        let mut dest_path_unicode:UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut dest_path_unicode, dest_path.as_ptr());

        let mut dst_obj_attr:OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES { 
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32, 
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &mut dest_path_unicode,
            Attributes: 0,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null()
        };

        let mut dest_status:IO_STATUS_BLOCK = Default::default();
        let mut dest_handle:HANDLE = Default::default();
        
        let nt_status = NtCreateFile(
            &mut dest_handle,
            GENERIC_WRITE,
            &mut dst_obj_attr,
            &mut dest_status,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            0,
            FILE_OVERWRITE_IF,
            FILE_NON_DIRECTORY_FILE,
            std::ptr::null(),
            0
        );
        
        if nt_status != STATUS_SUCCESS {
            NtClose(source_handle);
            return nt_status;
        }

        const BUFFER_SIZE: usize = 64 * 1024; // 64KB buffer
        let mut buffer: Vec<u8> = vec![0u8; BUFFER_SIZE];
        let mut bytes_remaining = file_size;
        let mut offset: i64 = 0;

        while bytes_remaining > 0 {
            let bytes_to_read = std::cmp::min(bytes_remaining, BUFFER_SIZE) as u32;
            let mut read_status: IO_STATUS_BLOCK = Default::default();
            
            let nt_status = NtReadFile(
                source_handle,
                std::ptr::null_mut(),
                None,
                std::ptr::null(),
                &mut read_status,
                buffer.as_mut_ptr() as *mut c_void,
                bytes_to_read,
                &offset,
                std::ptr::null()
            );
            
            if nt_status != STATUS_SUCCESS && nt_status != STATUS_PENDING {
                NtClose(source_handle);
                NtClose(dest_handle);
                return nt_status;
            }

            let bytes_read = read_status.Information as u32;
            if bytes_read == 0 {
                break;
            }

            let mut write_status: IO_STATUS_BLOCK = Default::default();
            let nt_status = NtWriteFile(
                dest_handle,
                std::ptr::null_mut(),
                None,
                std::ptr::null(),
                &mut write_status,
                buffer.as_ptr() as *const c_void,
                bytes_read,
                &offset,
                std::ptr::null()
            );
            
            if nt_status != STATUS_SUCCESS && nt_status != STATUS_PENDING {
                NtClose(source_handle);
                NtClose(dest_handle);
                return nt_status;
            }

            offset += bytes_read as i64;
            bytes_remaining -= bytes_read as usize;
        }

        NtClose(source_handle);
        NtClose(dest_handle);
        
        0
    }
}

/*
kill browsers + normal copy
*/
pub unsafe fn kill_module_processes(module_path:&str) {
    let module_name = module_path.rsplit('\\').next().unwrap_or("").to_lowercase();
    if !module_name.is_empty() {
        let matching_pids = processes(module_name);
        for kill_pid in matching_pids {
            let h = OpenProcess(PROCESS_TERMINATE, 0, kill_pid);
            if !h.is_null() {
                TerminateProcess(h, 1);
                CloseHandle(h);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

pub fn to_unc_utf16(path:&str) -> Vec<u16> {
    let unc_path = if !path.starts_with("\\??\\") {
        format!("\\??\\{}",&path)
    } else {
        String::from(path)
    };
    return unc_path.encode_utf16().chain(iter::once(0)).collect();
}

pub fn get_special_folder(csidl:u32) -> String {
    unsafe {
    let mut outbuf:Vec<u16> = Vec::with_capacity(MAX_PATH as usize);
    
    SHGetSpecialFolderPathW(std::ptr::null_mut(),outbuf.as_mut_ptr(),csidl as _,1);

    let mut index:usize = 0; 
    loop {
        if index >= MAX_PATH as usize {
            break
        }
        if *outbuf.as_ptr().add(index) == 0 {
            break
        }
        index+=1;
    }

    String::from_utf16(slice::from_raw_parts(outbuf.as_ptr(), index)).unwrap()
    }
} 

pub fn write_string_to_memory_as_utf16(out: *mut usize,out_size:*mut usize,rust_string:String) {
    unsafe {
    let as_utf16:Vec<u16> = rust_string.encode_utf16().collect();
    *out_size = as_utf16.len() * 2;
    
    let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
    
    ptr::copy_nonoverlapping(as_utf16.as_ptr() as *const u8, tmp, *out_size);
    ptr::write(out, tmp as usize);
    }
}