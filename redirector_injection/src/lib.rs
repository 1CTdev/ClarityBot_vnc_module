#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::{char::{DecodeUtf16, decode_utf16}, fs, iter, mem, ops::Index, ptr::{self, null_mut, write_unaligned}, slice, sync::{OnceLock, RwLock}, thread::sleep, time::Duration};
use inject::{create_remote_thread_with_event};
use win_utils::get_special_folder;
use std::{ffi::c_void, string};
use windows_sys::{
    Wdk::{Foundation::{NtQueryObject, OBJECT_ATTRIBUTES, OBJECT_INFORMATION_CLASS, OBJECT_NAME_INFORMATION}, Storage::FileSystem::{FILE_ALL_INFORMATION, FILE_BASIC_INFORMATION, FILE_DIRECTORY_FILE, FILE_INFORMATION_CLASS, FILE_NAME_INFORMATION, FILE_OPEN, FILE_OPEN_IF, FILE_SYNCHRONOUS_IO_NONALERT, FileAllInformation, FileAlternateNameInformation, FileBasicInformation, FileNameInformation, FileNormalizedNameInformation, NtCreateDirectoryObject, NtCreateFile, NtOpenDirectoryObject, NtOpenFile, NtQueryDirectoryFile, NtQueryInformationFile, RtlInitUnicodeStringEx}, System::Threading::NtQueryInformationProcess}, Win32::{
        Foundation::{CloseHandle, GENERIC_ALL, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, LocalFree, MAX_PATH, NTSTATUS, STATUS_SUCCESS, UNICODE_STRING, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT}, Security::SECURITY_ATTRIBUTES, Storage::FileSystem::{CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN, FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, SYNCHRONIZE, SetFilePointer}, System::{Console::AllocConsole, Diagnostics::Debug::{FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW, ReadProcessMemory}, IO::{IO_STATUS_BLOCK, PIO_APC_ROUTINE}, Kernel::LIST_ENTRY, LibraryLoader::{GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleFileNameW, GetModuleHandleExW, GetModuleHandleW, GetProcAddress, LoadLibraryW}, Memory::{MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAlloc, VirtualAllocEx, VirtualFree, VirtualProtect}, StationsAndDesktops::{CloseDesktop, GetThreadDesktop, GetUserObjectInformationW, OpenInputDesktop}, SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH, LANG_NEUTRAL, SUBLANG_DEFAULT}, Threading::{CREATE_PROCESS_LOGON_FLAGS, CREATE_SUSPENDED, CreateEventW, CreateProcessAsUserW, CreateProcessW, CreateProcessWithTokenW, CreateRemoteThread, CreateThread, EVENT_MODIFY_STATE, GetCurrentProcess, GetCurrentProcessId, OpenEventW, OpenProcess, PEB, PROCESS_BASIC_INFORMATION, PROCESS_CREATE_THREAD, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, ResumeThread, STARTUPINFOW, SetEvent, WaitForSingleObject}}, UI::{Shell::{CSIDL_ADMINTOOLS, SHGetSpecialFolderPathW}, WindowsAndMessaging::{MB_OK, MessageBoxW}}
    }, core::*
};


type FnNtOpenFile = unsafe extern "system" fn(
    FileHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut OBJECT_ATTRIBUTES,
    IoStatusBlock: *mut IO_STATUS_BLOCK,
    ShareAccess: u32,
    OpenOptions: u32,
) -> NTSTATUS;

type FnNtCreateFile = unsafe extern "system" fn(
    FileHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut OBJECT_ATTRIBUTES,
    IoStatusBlock: *mut IO_STATUS_BLOCK,
    AllocationSize: *mut i64,
    FileAttributes: u32,
    ShareAccess: u32,
    CreateDisposition: u32,
    CreateOptions: u32,
    EaBuffer: *mut c_void,
    EaLength: u32,
) -> NTSTATUS;

type FnNtOpenDirectoryObject = unsafe extern "system" fn(
directoryhandle:*mut HANDLE,
desiredaccess:u32,
objectattributes: *const OBJECT_ATTRIBUTES,
) -> NTSTATUS;

type FnNtCreateDirectoryObject = unsafe extern "system" fn(
directoryhandle: *mut HANDLE,
desiredaccess:u32,
objectattributes: *const OBJECT_ATTRIBUTES,
) -> NTSTATUS;

type FnNtQueryInformationFile = unsafe extern "system" fn(
    FileHandle: HANDLE,
    iostatusblock:*mut IO_STATUS_BLOCK,
    fileinformatio:*mut c_void,
    length:u32,
    fileinformationclass:FILE_INFORMATION_CLASS,
) -> NTSTATUS;

type FnNtQueryAttributesFile = unsafe extern "system" fn(
    ObjectAttributes:*mut OBJECT_ATTRIBUTES,
    FileInformation:*mut FILE_BASIC_INFORMATION,
) -> NTSTATUS;

type FnNtQueryDirectoryFile = unsafe extern "system" fn(
    filehandle:HANDLE,
    event:HANDLE,
    apcroutine:PIO_APC_ROUTINE,
    apccontext:*mut c_void,
    iostatusblock:*mut IO_STATUS_BLOCK,
    fileinformation:*mut c_void,
    length:u32,
    fileinformationclass:i32,
    returnsingleentry:bool,
    filename:*const UNICODE_STRING,
    restartscan:bool,
) -> NTSTATUS;

type FnCreateProcessW = unsafe extern "system" fn(
    lpapplicationname : windows_sys::core::PCWSTR,
    lpcommandline : windows_sys::core::PWSTR,
    lpprocessattributes : *const SECURITY_ATTRIBUTES,
    lpthreadattributes : *const SECURITY_ATTRIBUTES,
    binherithandles : windows_sys::core::BOOL,
    dwcreationflags : PROCESS_CREATION_FLAGS,
    lpenvironment : *const core::ffi::c_void,
    lpcurrentdirectory : windows_sys::core::PCWSTR,
    lpstartupinfo : *const STARTUPINFOW,
    lpprocessinformation : *mut PROCESS_INFORMATION,
) -> windows_sys::core::BOOL;

type FnCreateProcessAsUserW = unsafe extern "system" fn(
    htoken : HANDLE,
    lpapplicationname : windows_sys::core::PCWSTR,
    lpcommandline : windows_sys::core::PWSTR,
    lpprocessattributes : *const SECURITY_ATTRIBUTES,
    lpthreadattributes : *const SECURITY_ATTRIBUTES,
    binherithandles : windows_sys::core::BOOL,
    dwcreationflags : PROCESS_CREATION_FLAGS,
    lpenvironment : *const core::ffi::c_void,
    lpcurrentdirectory : windows_sys::core::PCWSTR,
    lpstartupinfo : *const STARTUPINFOW,
    lpprocessinformation : *mut PROCESS_INFORMATION
) -> windows_sys::core::BOOL;

type FnCreateProcessWithTokenW = unsafe extern "system" fn(
    htoken : HANDLE,
    dwlogonflags : CREATE_PROCESS_LOGON_FLAGS,
    lpapplicationname : windows_sys::core::PCWSTR,
    lpcommandline : windows_sys::core::PWSTR,
    dwcreationflags : PROCESS_CREATION_FLAGS,
    lpenvironment : *const core::ffi::c_void,
    lpcurrentdirectory : windows_sys::core::PCWSTR,
    lpstartupinfo : *const STARTUPINFOW,
    lpprocessinformation : *mut PROCESS_INFORMATION
) -> windows_sys::core::BOOL;

type FnNtQueryObject = unsafe extern "system" fn (
    handle: HANDLE,
    objectinformationclass:OBJECT_INFORMATION_CLASS,
    objectinformation:*mut core::ffi::c_void,
    objectinformationlength:u32,
    returnlength: *mut u32,
) -> NTSTATUS;

type FnNtUserGetObjectInformation = unsafe extern "system" fn (
    hobj:HANDLE,nindex:i32,pvinfo: *mut c_void,nlength:u32,ipnlengthneeded:*mut u32
) -> i32;

//fs apis
//object_attributes based functions
static mut NT_CREATE_FILE_GATEWAY_ADDR:usize= 0;
static mut NT_OPEN_FILE_GATEWAY_ADDR:usize = 0;
static mut NT_OPEN_DIRECTORY_OBJECT_GATEWAY_ADDR:usize = 0;
static mut NT_CREATE_DIRECTORY_OBJECT_GATEWAY_ADDR:usize = 0;
static mut NT_QUERY_ATTRIBUTES_FILE_GATEWAY_ADDR:usize = 0;

static mut NT_QUERY_INFORMATION_FILE_GATEWAY_ADDR:usize = 0;
static mut NT_QUERY_DIRECTORY_FILE_GATEWAY_ADDR:usize = 0;
static mut NT_QUERY_OBJECT_GATEWAY_ADDR:usize = 0;


//process apis
static mut CREATE_PROCESS_W_GATEWAY_ADDR:usize = 0;
static mut CREATE_PROCESS_AS_USER_W_GATEWAY_ADDR:usize = 0;
static mut CREATE_PROCESS_WITH_TOKEN_W_GATEWAY_ADDR:usize = 0;

static mut NT_USER_GET_OBJECT_INFORMATION_GATEWAY_ADDR:usize = 0;

//dldr meaning dream loader, we choose a folder with same length as original to avoid needing to reallocate data, instead just replacing bytes in the output of certain apis
/*
static to_redirect:&str = r#"C:\Users\user\AppData\Local\Microsoft\Edge"#;
static redirect_to:&str = r#"C:\Users\user\AppData\Local\Microsoft\dldr"#;
static redirect_to_replace:&str = r#"\Edge\"#;
static redirect_replace_with:&str = r#"\dldr\"#;
*/
static TO_REDIRECT:OnceLock<String> = OnceLock::new();
static REDIRECT_TO:OnceLock<String> = OnceLock::new();
static REDIRECT_TO_REPLACE:OnceLock<String> = OnceLock::new();
static REDIRECT_REPLACE_WITH:OnceLock<String> = OnceLock::new();

//static mut NtUserGetObjectInformation:usize = 0;

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(base: usize, reason: u32, _: u32) -> i32 {
    unsafe {
        match reason {
            DLL_PROCESS_ATTACH => {
                let mut threadid: u32 = 0;
                let hthread = CreateThread(
                    std::ptr::null(),
                    0,
                    Some(entry_thread),
                    base as *const c_void,
                    0,
                    &mut threadid,
                );
            }
            DLL_PROCESS_DETACH => {}
            DLL_THREAD_ATTACH => {}
            DLL_THREAD_DETACH => {}
            _ => {}
        }
    }
    1
}
/*
unsafe fn setup_redirect_directory() {
    unsafe {
        let ntdll = GetModuleHandleW(w!("ntdll.dll"));
        if ntdll.is_null() {
            return;
        }

        let NtQueryAttributesFile:FnNtQueryAttributesFile = std::mem::transmute(GetProcAddress(ntdll, s!("NtQueryAttributesFile")).unwrap());
    

        let mut scoped_to_redirect = TO_REDIRECT.get().unwrap().clone();
        if !scoped_to_redirect.starts_with(r#"\??\"#) {
            scoped_to_redirect = format!("{}{}",r#"\??\"#,scoped_to_redirect.to_string());
        }
        
        let mut scoped_to_redirect:Vec<u16> = scoped_to_redirect.encode_utf16().chain(iter::once(0)).collect();

        let mut to_redirect_unicode: UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut to_redirect_unicode, scoped_to_redirect.as_mut_ptr());

        let mut obj_attr:OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES{
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32, 
            RootDirectory:std::ptr::null_mut(),
            ObjectName:&mut to_redirect_unicode,
            Attributes:0,
            SecurityDescriptor:std::ptr::null_mut(), 
            SecurityQualityOfService:std::ptr::null_mut()
        };

        let mut fi:FILE_BASIC_INFORMATION = mem::zeroed();
        let status = NtQueryAttributesFile(&mut obj_attr,&mut fi);
        if status != STATUS_SUCCESS {
            #[cfg(debug_assertions)]
            {
                win_utils::print_last_error(Some(String::from("NtQueryAttributesFile")));
            }
            return
        }

        let mut scoped_redirect_to = REDIRECT_TO.get().unwrap().clone();
        if !scoped_redirect_to.starts_with(r#"\??\"#) {
            scoped_redirect_to = format!("{}{}",r#"\??\"#,scoped_redirect_to);
        }


        let mut scoped_redirect_to:Vec<u16> = scoped_redirect_to.encode_utf16().chain(iter::once(0)).collect();

        let mut redirect_to_unicode: UNICODE_STRING = mem::zeroed();
        RtlInitUnicodeStringEx(&mut redirect_to_unicode, scoped_redirect_to.as_mut_ptr());

        let obj_attr: OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES{
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32, 
            RootDirectory:std::ptr::null_mut(),
            ObjectName:&mut redirect_to_unicode,
            Attributes:0,
            SecurityDescriptor:std::ptr::null_mut(), 
            SecurityQualityOfService:std::ptr::null_mut()
        };

        let mut hfile:HANDLE = Default::default();
        let mut io_status_block:IO_STATUS_BLOCK = mem::zeroed();
        let status = NtCreateFile(&mut hfile,
            FILE_GENERIC_WRITE | SYNCHRONIZE,
            &obj_attr as _,
            &mut io_status_block,
            std::ptr::null(),
            fi.FileAttributes,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_OPEN_IF,
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        );

        if status != STATUS_SUCCESS {
            #[cfg(debug_assertions)]
            {
                win_utils::print_last_error(Some(String::from("NtCreateFile")));
            }
        } else {
            CloseHandle(hfile);
        }
    }
}
*/

fn initalize_redirection_settings() {
        let output_dir = get_special_folder(CSIDL_ADMINTOOLS);
        let redirect_dll_path:String = format!("{}\\{}",output_dir,"redi.dll");
        let redirect_txt_path:String = format!("{}\\{}",output_dir,"redirect.txt");
        #[cfg(debug_assertions)]
        {
            println!("{}",redirect_dll_path);
            println!("{}",redirect_txt_path);
        }
        
        let mut output = fs::read(redirect_txt_path).unwrap();
  
        let contents = unsafe {String::from_raw_parts(output.as_mut_ptr(), output.len(), output.capacity()) };

        for line in contents.lines() {
            if line.starts_with("to_redirect=") {
                TO_REDIRECT.set(line.replace("to_redirect=",""));
            } else if line.starts_with("redirect_to=") {
                REDIRECT_TO.set(line.replace("redirect_to=",""));
            } else if line.starts_with("redirect_to_replace=") {
                REDIRECT_TO_REPLACE.set(line.replace("redirect_to_replace=", ""));
            } else if line.starts_with("redirect_replace_with=") {
                REDIRECT_REPLACE_WITH.set(line.replace("redirect_replace_with=",""));
            }
        }
        std::mem::forget(output_dir);
        std::mem::forget(output);
        std::mem::forget(contents);
}

#[unsafe(no_mangle)]
unsafe extern "system" fn entry_thread(_param: *mut c_void) -> u32 {
    unsafe {

        #[cfg(debug_assertions)] 
        {
           let _ = AllocConsole();
        }

        initalize_redirection_settings();

        //setup_redirect_directory();

        let ntdll = GetModuleHandleW(w!("ntdll.dll"));
        if ntdll.is_null() {
            return 0;
        }

        let kernelbase = GetModuleHandleW(w!("kernelbase.dll"));
        if kernelbase.is_null() {
            return 0;
        }

        let nt_open_file_addr:usize = std::mem::transmute(GetProcAddress(ntdll, s!("NtOpenFile")).unwrap());
        let nt_create_file_addr:usize = std::mem::transmute(GetProcAddress(ntdll, s!("NtCreateFile")).unwrap());
        let nt_open_directory_object_addr:usize = std::mem::transmute(GetProcAddress(ntdll,s!("NtOpenDirectoryObject")).unwrap());
        let nt_create_directory_object_addr:usize = std::mem::transmute(GetProcAddress(ntdll,s!("NtCreateDirectoryObject")).unwrap());
        let nt_query_information_file_addr:usize = std::mem::transmute(GetProcAddress(ntdll,s!("NtQueryInformationFile")));
        let nt_query_directory_file_addr:usize = std::mem::transmute(GetProcAddress(ntdll, s!("NtQueryDirectoryFile")));
        let nt_query_attributes_file_addr:usize = std::mem::transmute(GetProcAddress(ntdll,s!("NtQueryAttributesFile")));
        let nt_query_object_addr:usize = std::mem::transmute(GetProcAddress(ntdll,s!("NtQueryObject")));
        
        let create_process_w_addr:usize = std::mem::transmute(GetProcAddress(kernelbase,s!("CreateProcessW")).unwrap());

        NT_CREATE_FILE_GATEWAY_ADDR = detour::hook(nt_create_file_addr,NtCreateFile_hook as usize);
        NT_OPEN_FILE_GATEWAY_ADDR = detour::hook(nt_open_file_addr,NtOpenFile_hook as usize);
        NT_OPEN_DIRECTORY_OBJECT_GATEWAY_ADDR = detour::hook(nt_open_directory_object_addr,NtOpenDirectoryObject_hook as usize);
        NT_CREATE_DIRECTORY_OBJECT_GATEWAY_ADDR = detour::hook(nt_create_directory_object_addr,NtCreateDirectoryObject_hook as usize);

        NT_QUERY_INFORMATION_FILE_GATEWAY_ADDR = detour::hook(nt_query_information_file_addr,NtQueryInformationFile_hook as usize);
        NT_QUERY_DIRECTORY_FILE_GATEWAY_ADDR = detour::hook(nt_query_directory_file_addr, NtQueryDirectoryFile_hook as usize);
        NT_QUERY_ATTRIBUTES_FILE_GATEWAY_ADDR = detour::hook(nt_query_attributes_file_addr,NtQueryAttributesFile_hook as usize);
        NT_QUERY_OBJECT_GATEWAY_ADDR = detour::hook(nt_query_object_addr,NtQueryObject_hook as usize);
        
        CREATE_PROCESS_W_GATEWAY_ADDR = detour::hook(create_process_w_addr,CreateProcessW_hook as usize);

        // Signal the init event to let the injector know we're done
        let event_name: Vec<u16> = format!("Global\\dldr_{}", GetCurrentProcessId())
            .encode_utf16()
            .chain(iter::once(0))
            .collect();
        let hevent = OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr());
        if !hevent.is_null() {
            SetEvent(hevent);
            CloseHandle(hevent);

            #[cfg(debug_assertions)]
            {
                println!("[HOOK] Signaled init complete event");
            }
        }
        0
    }
}

unsafe fn create_new_objectattributes(ObjectAttributes: *mut OBJECT_ATTRIBUTES) -> Option<OBJECT_ATTRIBUTES> {

    /*
    object_name will be normalized:
    .\AppData\Local\Microsoft\Edge\test3.txt -> AppData\Local\Microsoft\Edge\test3.txt

    there will be no path separater between the root dir and object_name, we will need to join and format the path ourselves.

            //need to handle this case
        //rootdir is not null when the path supplied to the higher level functions is a relative path
        //returns device path, not volume symbol
     */

    let rootdirectory = if !(*ObjectAttributes).RootDirectory.is_null() {
        let mut out:Vec<u8> = Vec::with_capacity(1024);
        let mut out_len:u32 = 0;
        
        let status = if NT_QUERY_OBJECT_GATEWAY_ADDR == 0 {
            NtQueryObject((*ObjectAttributes).RootDirectory,
            1, //ObjectNameInformation
            out.as_mut_ptr() as _,
            out.capacity() as u32,
            &mut out_len
            )
        } else {
            let NtQueryObject:FnNtQueryObject = std::mem::transmute(NT_QUERY_OBJECT_GATEWAY_ADDR);
            NtQueryObject((*ObjectAttributes).RootDirectory,
            1, //ObjectNameInformation
            out.as_mut_ptr() as _,
            out.capacity() as u32,
            &mut out_len
            )
        };

        if status != STATUS_SUCCESS {
            #[cfg(debug_assertions)]
            {
                win_utils::print_last_error(Some(String::from("NtQueryObject")));
            }
            return None;
        }

        out.set_len(out_len as usize);

        let name_info = out.as_mut_ptr().cast::<OBJECT_NAME_INFORMATION>();
        let rootdirectory = String::from_utf16(slice::from_raw_parts((*name_info).Name.Buffer,(*name_info).Name.Length as usize/2)).unwrap();
        Some(rootdirectory)
    } else {
        None
    };

    if !(*(*ObjectAttributes).ObjectName).Buffer.is_null() {
        let object_name = String::from_utf16(slice::from_raw_parts((*(*ObjectAttributes).ObjectName).Buffer, (*(*ObjectAttributes).ObjectName).Length as usize/2)).unwrap();

        let fullpath:String = match rootdirectory {
            Some(rootdirectory) => {
                let mappings = win_utils::get_drive_mappings();
                let dos_path = win_utils::device_path_to_dos_path(rootdirectory.as_str(), &mappings).unwrap();
                format!(r#"{}\{}"#,dos_path,object_name)
            },
            None => {
                object_name
            },
        };

        #[cfg(debug_assertions)]
        {
            println!("original object name: {}",fullpath);
        }

        if fullpath.to_lowercase().contains(&TO_REDIRECT.get().unwrap().to_lowercase()) {
            let new_object_name = fullpath.replace(REDIRECT_TO_REPLACE.get().unwrap(), &REDIRECT_REPLACE_WITH.get().unwrap());
            
            #[cfg(debug_assertions)]
            {
                println!("new object name: {}",new_object_name);
            }

            let mut new_object_name_wide: Vec<u16> = new_object_name.encode_utf16().chain(iter::once(0)).collect();
            let tmp = VirtualAlloc(std::ptr::null(), new_object_name_wide.len()*2, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
            ptr::copy_nonoverlapping(new_object_name_wide.as_mut_ptr() as _, tmp, new_object_name_wide.len()*2);

            let mut redirect_to_unicode: UNICODE_STRING = mem::zeroed();
            RtlInitUnicodeStringEx(&mut redirect_to_unicode, tmp as _);
            

            let new_obj_attr:OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES { Length:size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory:std::ptr::null_mut(),
            ObjectName: &redirect_to_unicode, 
            Attributes: (*ObjectAttributes).Attributes,
            SecurityDescriptor:(*ObjectAttributes).SecurityDescriptor,
            SecurityQualityOfService:(*ObjectAttributes).SecurityQualityOfService 
            };

            return Some(new_obj_attr)
        }
    }
    None
}


#[unsafe(no_mangle)]
pub unsafe extern "system" fn NtOpenFile_hook(
    FileHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut OBJECT_ATTRIBUTES,
    IoStatusBlock: *mut IO_STATUS_BLOCK,
    ShareAccess: u32,
    OpenOptions: u32,
) -> NTSTATUS {
    let gateway:FnNtOpenFile = std::mem::transmute(NT_OPEN_FILE_GATEWAY_ADDR);

    #[cfg(debug_assertions)]
    {
        println!("[HOOK] NtOpenFile called");
    }

    match create_new_objectattributes(ObjectAttributes) {
        Some(mut new_obj_attr) => {
            let status = gateway(FileHandle,DesiredAccess,&mut new_obj_attr,IoStatusBlock,ShareAccess,OpenOptions);
            if VirtualFree((*new_obj_attr.ObjectName).Buffer as _, 0, MEM_RELEASE) == 0 {

                #[cfg(debug_assertions)]
                {
                    win_utils::print_last_error(Some(String::from("VirtualFree")));
                }
            };
            return status;
        },
        None => {
            return gateway(FileHandle,DesiredAccess,ObjectAttributes,IoStatusBlock,ShareAccess,OpenOptions);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn NtCreateFile_hook(
    FileHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: *mut OBJECT_ATTRIBUTES,
    IoStatusBlock: *mut IO_STATUS_BLOCK,
    AllocationSize: *mut i64,
    FileAttributes: u32,
    ShareAccess: u32,
    CreateDisposition: u32,
    CreateOptions: u32,
    EaBuffer: *mut c_void,
    EaLength: u32,
) -> NTSTATUS {
    let gateway:FnNtCreateFile = std::mem::transmute(NT_CREATE_FILE_GATEWAY_ADDR);

    #[cfg(debug_assertions)]
    {
        println!("[HOOK] NtCreateFile called");
    }

    match create_new_objectattributes(ObjectAttributes) {
        Some(mut new_obj_attr) => {
            let status = gateway(FileHandle,DesiredAccess,&mut new_obj_attr,IoStatusBlock,AllocationSize,FileAttributes,ShareAccess,CreateDisposition,CreateOptions,EaBuffer,EaLength);
            if VirtualFree((*new_obj_attr.ObjectName).Buffer as _, 0, MEM_RELEASE) == 0 {
                #[cfg(debug_assertions)]
                {
                    win_utils::print_last_error(Some(String::from("VirtualFree")));
                }
            };
            return status;
        },
        None => {
            return gateway(FileHandle,DesiredAccess,ObjectAttributes,IoStatusBlock,AllocationSize,FileAttributes,ShareAccess,CreateDisposition,CreateOptions,EaBuffer,EaLength);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn NtOpenDirectoryObject_hook(
    directoryhandle:*mut HANDLE,
    desiredaccess:u32,
    objectattributes: *mut OBJECT_ATTRIBUTES,
) -> NTSTATUS {
    unsafe {
        let gateway:FnNtOpenDirectoryObject = std::mem::transmute(NT_OPEN_DIRECTORY_OBJECT_GATEWAY_ADDR);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] NtOpenDirectoryObject called");
        }

        match create_new_objectattributes(objectattributes) {
            Some(mut new_obj_attr) => {
                let status = gateway(directoryhandle,desiredaccess,&mut new_obj_attr);
                if VirtualFree((*new_obj_attr.ObjectName).Buffer as _, 0, MEM_RELEASE) == 0 {

                    #[cfg(debug_assertions)]
                    {
                        win_utils::print_last_error(Some(String::from("VirtualFree")));
                    }
                };
                return status;
            },
            None => {
                return gateway(directoryhandle,desiredaccess,objectattributes);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn NtCreateDirectoryObject_hook(
directoryhandle: *mut HANDLE,
desiredaccess:u32,
objectattributes: *mut OBJECT_ATTRIBUTES,
) -> NTSTATUS {
    unsafe {
        let gateway:FnNtCreateDirectoryObject = std::mem::transmute(NT_CREATE_DIRECTORY_OBJECT_GATEWAY_ADDR);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] NtCreateDirectoryObject called");
        }

        match create_new_objectattributes(objectattributes) {
            Some(mut new_obj_attr) => {
                let status = gateway(directoryhandle,desiredaccess,&mut new_obj_attr);
                if VirtualFree((*new_obj_attr.ObjectName).Buffer as _, 0, MEM_RELEASE) == 0 {

                    #[cfg(debug_assertions)]
                    {
                        win_utils::print_last_error(Some(String::from("VirtualFree")));
                    }
                };
                return status;
            },
            None => {
                return gateway(directoryhandle,desiredaccess,objectattributes);
            }
        }
    }
}

pub unsafe extern "system" fn NtQueryAttributesFile_hook(
    ObjectAttributes:*mut OBJECT_ATTRIBUTES,
    FileInformation:*mut FILE_BASIC_INFORMATION,
) -> NTSTATUS {
    unsafe {
        let gateway:FnNtQueryAttributesFile = std::mem::transmute(NT_QUERY_ATTRIBUTES_FILE_GATEWAY_ADDR);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] NtQueryAttributesFile called");
        }

        match create_new_objectattributes(ObjectAttributes) {
        Some(mut new_obj_attr) => {
            let status = gateway(&mut new_obj_attr,FileInformation);
            if VirtualFree((*new_obj_attr.ObjectName).Buffer as _, 0, MEM_RELEASE) == 0 {

                #[cfg(debug_assertions)] 
                {
                    win_utils::print_last_error(Some(String::from("VirtualFree")));
                }
            };
            return status;
        },
        None => {
            return gateway(ObjectAttributes,FileInformation);
        }
    }
    }
}

const FileShortNameInformation:i32 = 40;

/// Replaces redirect_replace_with with redirect_to_replace in a FILE_NAME_INFORMATION buffer in-place
/// Since "Edge" and "dldr" have the same length, we can do a direct byte replacement
unsafe fn fixup_file_name_information(file_name_info: *mut FILE_NAME_INFORMATION) {
    unsafe {
        let name_len = (*file_name_info).FileNameLength as usize / 2; // Length in characters
        if name_len == 0 {
            return;
        }
        
        let file_name = String::from_utf16_lossy(
            slice::from_raw_parts((*file_name_info).FileName.as_ptr(), name_len)
        );
        
        // Check if the path contains our redirect target (dldr)
        if file_name.to_lowercase().contains(&REDIRECT_REPLACE_WITH.get().unwrap().to_lowercase()) {
            // Replace dldr with Edge
            let new_name = file_name.replace(REDIRECT_REPLACE_WITH.get().unwrap(), &REDIRECT_TO_REPLACE.get().unwrap());
            let new_name_wide: Vec<u16> = new_name.encode_utf16().collect();
            
            // Since both strings have the same length, we can do in-place replacement
            if new_name_wide.len() == name_len {

                #[cfg(debug_assertions)]
                {
                    println!("[HOOK] NtQueryInformationFile: Fixing path from '{}' to '{}'", file_name, new_name);
                }

                ptr::copy_nonoverlapping(
                    new_name_wide.as_ptr(),
                    (*file_name_info).FileName.as_mut_ptr(),
                    new_name_wide.len()
                );
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn NtQueryInformationFile_hook(
    FileHandle: HANDLE,
    iostatusblock:*mut IO_STATUS_BLOCK,
    fileinformation:*mut c_void,
    length:u32,
    fileinformationclass:FILE_INFORMATION_CLASS,
) -> NTSTATUS {
    unsafe {
        let gateway:FnNtQueryInformationFile = std::mem::transmute(NT_QUERY_INFORMATION_FILE_GATEWAY_ADDR);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] NtQueryInformationFile called: filehandle:{:x} fileinformationclass:{}",FileHandle as usize,fileinformationclass);
        }
        
        // Call the original function first
        let status = gateway(FileHandle, iostatusblock, fileinformation, length, fileinformationclass);
        
        if status != STATUS_SUCCESS {
            return status;
        }
        
        // Handle file name information classes - these return FILE_NAME_INFORMATION structure
        // FileNameInformation = 9, FileAlternateNameInformation = 21, FileShortNameInformation = 40, FileNormalizedNameInformation = 48
        if fileinformationclass == FileNameInformation ||
           fileinformationclass == FileAlternateNameInformation ||
           fileinformationclass == FileShortNameInformation ||
           fileinformationclass == FileNormalizedNameInformation {
            let file_name_info = fileinformation.cast::<FILE_NAME_INFORMATION>();
            fixup_file_name_information(file_name_info);
        } else if fileinformationclass == FileAllInformation {
            // FileAllInformation includes NameInformation at offset
            let file_all_info = fileinformation.cast::<FILE_ALL_INFORMATION>();
            let name_info_ptr = &mut (*file_all_info).NameInformation as *mut FILE_NAME_INFORMATION;
            fixup_file_name_information(name_info_ptr);
        }
        
        status
    }
}


pub unsafe extern "system" fn NtQueryDirectoryFile_hook(
    filehandle:HANDLE,
    event:HANDLE,
    apcroutine:PIO_APC_ROUTINE,
    apccontext:*mut c_void,
    iostatusblock:*mut IO_STATUS_BLOCK,
    fileinformation:*mut c_void,
    length:u32,
    fileinformationclass:i32,
    returnsingleentry:bool,
    filename:*const UNICODE_STRING,
    restartscan:bool,
) -> NTSTATUS {
    unsafe {
        let gateway:FnNtQueryDirectoryFile = std::mem::transmute(NT_QUERY_DIRECTORY_FILE_GATEWAY_ADDR);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] NtQueryDirectoryFile called");
        }
        
        gateway(filehandle,event,apcroutine,apccontext,iostatusblock,fileinformation,length,fileinformationclass,returnsingleentry,filename,restartscan)
    }
}

const ObjectNameInformation:OBJECT_INFORMATION_CLASS = 1;

pub unsafe extern "system" fn NtQueryObject_hook(
    handle: HANDLE,
    objectinformationclass:OBJECT_INFORMATION_CLASS,
    objectinformation:*mut core::ffi::c_void,
    objectinformationlength:u32,
    returnlength: *mut u32,
) -> NTSTATUS {
    unsafe {
        let gateway:FnNtQueryObject = std::mem::transmute(NT_QUERY_OBJECT_GATEWAY_ADDR);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] NtQueryObject called: objectinformationclass:{}", objectinformationclass);
        }
        
        let status = gateway(handle, objectinformationclass, objectinformation, objectinformationlength, returnlength);
        
        if status != STATUS_SUCCESS {
            return status;
        }
        
        if objectinformationclass == ObjectNameInformation && !objectinformation.is_null() {
            let name_info = objectinformation.cast::<OBJECT_NAME_INFORMATION>();
            let name_len = (*name_info).Name.Length as usize / 2;
            
            if name_len > 0 && !(*name_info).Name.Buffer.is_null() {
                let object_name = String::from_utf16_lossy(
                    slice::from_raw_parts((*name_info).Name.Buffer, name_len)
                );

                if object_name.to_lowercase().contains(&REDIRECT_REPLACE_WITH.get().unwrap().to_lowercase()) {
                    let new_name = object_name.replace(REDIRECT_REPLACE_WITH.get().unwrap(), &REDIRECT_TO_REPLACE.get().unwrap());
                    let new_name_wide: Vec<u16> = new_name.encode_utf16().collect();

                    if new_name_wide.len() == name_len {
                        #[cfg(debug_assertions)]
                        {
                            println!("[HOOK] NtQueryObject: Fixing path from '{}' to '{}'", object_name, new_name);
                        }
                        ptr::copy_nonoverlapping(
                            new_name_wide.as_ptr(),
                            (*name_info).Name.Buffer,
                            new_name_wide.len()
                        );
                    }
                }
            }
        }
        
        status
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn CreateProcessW_hook(
    lpapplicationname : windows_sys::core::PCWSTR,
    lpcommandline : windows_sys::core::PWSTR,
    lpprocessattributes : *const SECURITY_ATTRIBUTES,
    lpthreadattributes : *const SECURITY_ATTRIBUTES,
    binherithandles : windows_sys::core::BOOL,
    dwcreationflags : PROCESS_CREATION_FLAGS,
    lpenvironment : *const core::ffi::c_void,
    lpcurrentdirectory : windows_sys::core::PCWSTR,
    lpstartupinfo : *const STARTUPINFOW,
    lpprocessinformation : *mut PROCESS_INFORMATION,
) -> windows_sys::core::BOOL {
    unsafe {
        let gateway:FnCreateProcessW = std::mem::transmute(CREATE_PROCESS_W_GATEWAY_ADDR);
        let application_name = win_utils::PCWSTR_to_string(lpapplicationname);
        let command_line = win_utils::PCWSTR_to_string(lpcommandline);

        #[cfg(debug_assertions)] 
        {
            println!("[HOOK] CreateProcessW called: applicationname:{:?} commandline:{:?}",application_name,command_line);
        }

        //could also just get the modulename of our current main module
        if application_name.ends_with("msedge.exe") ||
        command_line.contains("msedge.exe") || 
        application_name.ends_with("chrome.exe") ||
        command_line.contains("chrome.exe") ||
        application_name.ends_with("brave.exe") ||
        command_line.contains("brave.exe") {
            // Check if caller already wanted suspended - if so, don't resume after injection
            let caller_wanted_suspended = (dwcreationflags & CREATE_SUSPENDED) != 0;
            
            let status = gateway(lpapplicationname,lpcommandline,lpprocessattributes,lpthreadattributes,binherithandles,dwcreationflags|CREATE_SUSPENDED,lpenvironment,lpcurrentdirectory,lpstartupinfo,lpprocessinformation);
            if status != 0 {
                let mut hmod:HANDLE = Default::default();
                let local:*const u16 = &0xDEAD;
                GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,local,&mut hmod);
                
                let mut module_file_path:Vec<u16> = Vec::with_capacity(512);
                let written = GetModuleFileNameW(hmod,module_file_path.as_mut_ptr(),256);
                module_file_path.set_len(written as usize);
                let module_file_path = String::from_utf16(slice::from_raw_parts(module_file_path.as_mut_ptr(), written as usize)).unwrap();
                
                create_remote_thread_with_event(module_file_path,(*lpprocessinformation).dwProcessId);
                
                // Only resume if the caller didn't originally want it suspended
                if !caller_wanted_suspended {
                    let status = ResumeThread((*lpprocessinformation).hThread);
                    if status == 0xffffffff {
                        #[cfg(debug_assertions)]
                        {
                            win_utils::print_last_error(Some(String::from("ResumeThread")));
                        }
                    }
                }
            } else {
                #[cfg(debug_assertions)]
                {
                    win_utils::print_last_error(Some(String::from("CreateProcessW")));
                }
            }

            return status;
        } else {
            return gateway(lpapplicationname,lpcommandline,lpprocessattributes,lpthreadattributes,binherithandles,dwcreationflags,lpenvironment,lpcurrentdirectory,lpstartupinfo,lpprocessinformation);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn CreateProcessAsUserW_hook(
    htoken : HANDLE,
    lpapplicationname : windows_sys::core::PCWSTR,
    lpcommandline : windows_sys::core::PWSTR,
    lpprocessattributes : *const SECURITY_ATTRIBUTES,
    lpthreadattributes : *const SECURITY_ATTRIBUTES,
    binherithandles : windows_sys::core::BOOL,
    dwcreationflags : PROCESS_CREATION_FLAGS,
    lpenvironment : *const core::ffi::c_void,
    lpcurrentdirectory : windows_sys::core::PCWSTR,
    lpstartupinfo : *const STARTUPINFOW,
    lpprocessinformation : *mut PROCESS_INFORMATION
) -> windows_sys::core::BOOL {
    unsafe {
        let gateway:FnCreateProcessAsUserW = std::mem::transmute(CREATE_PROCESS_AS_USER_W_GATEWAY_ADDR);
        let application_name = win_utils::PCWSTR_to_string(lpapplicationname);
        let command_line = win_utils::PCWSTR_to_string(lpcommandline);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] CreateProcessAsUserW called: applicationname:{:?} commandline:{:?}",application_name,command_line);
        }

        gateway(htoken,lpapplicationname,lpcommandline,lpprocessattributes,lpthreadattributes,binherithandles,dwcreationflags,lpenvironment,lpcurrentdirectory,lpstartupinfo,lpprocessinformation)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn CreateProcessWithTokenW_hook(
    htoken : HANDLE,
    dwlogonflags : CREATE_PROCESS_LOGON_FLAGS,
    lpapplicationname : windows_sys::core::PCWSTR,
    lpcommandline : windows_sys::core::PWSTR,
    dwcreationflags : PROCESS_CREATION_FLAGS,
    lpenvironment : *const core::ffi::c_void,
    lpcurrentdirectory : windows_sys::core::PCWSTR,
    lpstartupinfo : *const STARTUPINFOW,
    lpprocessinformation : *mut PROCESS_INFORMATION
) -> windows_sys::core::BOOL {
    unsafe {
        let gateway:FnCreateProcessWithTokenW = std::mem::transmute(CREATE_PROCESS_WITH_TOKEN_W_GATEWAY_ADDR);
        let application_name = win_utils::PCWSTR_to_string(lpapplicationname);
        let command_line = win_utils::PCWSTR_to_string(lpcommandline);

        #[cfg(debug_assertions)]
        {
            println!("[HOOK] CreateProcessWithTokenW called: applicationname:{:?} commandline:{:?}",application_name,command_line);
        }

        gateway(htoken,dwlogonflags,lpapplicationname,lpcommandline,dwcreationflags,lpenvironment,lpcurrentdirectory,lpstartupinfo,lpprocessinformation)
    }
}