use iced_x86::*;

use windows_sys::{
    Wdk::{Foundation::{NtQueryObject, OBJECT_ATTRIBUTES, OBJECT_INFORMATION_CLASS, OBJECT_NAME_INFORMATION}, Storage::FileSystem::{FILE_ALL_INFORMATION, FILE_BASIC_INFORMATION, FILE_DIRECTORY_FILE, FILE_INFORMATION_CLASS, FILE_NAME_INFORMATION, FILE_OPEN, FILE_OPEN_IF, FILE_SYNCHRONOUS_IO_NONALERT, FileAllInformation, FileAlternateNameInformation, FileBasicInformation, FileNameInformation, FileNormalizedNameInformation, NtCreateDirectoryObject, NtCreateFile, NtOpenDirectoryObject, NtOpenFile, NtQueryDirectoryFile, NtQueryInformationFile, RtlInitUnicodeStringEx}, System::Threading::NtQueryInformationProcess}, Win32::{
        Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, LocalFree, MAX_PATH, NTSTATUS, STATUS_SUCCESS, UNICODE_STRING, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT}, Security::SECURITY_ATTRIBUTES, Storage::FileSystem::{CREATE_ALWAYS, CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN, FILE_GENERIC_WRITE, FILE_LIST_DIRECTORY, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, SYNCHRONIZE, SetFilePointer}, System::{Console::AllocConsole, Diagnostics::Debug::{FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW, ReadProcessMemory}, IO::{IO_STATUS_BLOCK, PIO_APC_ROUTINE}, Kernel::LIST_ENTRY, LibraryLoader::{GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleFileNameW, GetModuleHandleExW, GetModuleHandleW, GetProcAddress, LoadLibraryW}, Memory::{MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAlloc, VirtualAllocEx, VirtualFree, VirtualProtect}, SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH, LANG_NEUTRAL, SUBLANG_DEFAULT}, Threading::{CREATE_PROCESS_LOGON_FLAGS, CREATE_SUSPENDED, CreateEventW, CreateProcessAsUserW, CreateProcessW, CreateProcessWithTokenW, CreateRemoteThread, CreateThread, EVENT_MODIFY_STATE, GetCurrentProcess, GetCurrentProcessId, OpenEventW, OpenProcess, PEB, PROCESS_BASIC_INFORMATION, PROCESS_CREATE_THREAD, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, ResumeThread, STARTUPINFOW, SetEvent, WaitForSingleObject}}, UI::{Shell::{CSIDL_ADMINTOOLS, SHGetSpecialFolderPathW}, WindowsAndMessaging::{MB_OK, MessageBoxW}}
    }, core::*
};


fn instr_to_overwrite(func_addr:usize,hook_len:usize) -> usize {
    unsafe {
        let mut instructions:[u8;32] = [0;32];
         
        std::ptr::copy(func_addr as _, instructions.as_mut_ptr(), instructions.len());

        let mut decoder = Decoder::with_ip(64, &instructions, 0, DecoderOptions::NONE);

        let mut i:usize = 0;
        for instr in &mut decoder {
            i += instr.len();
            if i >= hook_len {
                return i;
            }
        }
    }
    0
}

pub fn hook(address:usize,myfunction:usize) -> usize {
    #[cfg(debug_assertions)]
    {
        println!("hooking {:x} with {:x}",address,myfunction);
    }

    unsafe {
    let original_bytes_len = instr_to_overwrite(address, 14);
    
    let mut original_protection:u32 = 0;
    VirtualProtect(address as _,original_bytes_len,PAGE_EXECUTE_READWRITE,&mut original_protection);
    
    let mut original_bytes:Vec<u8> = Vec::with_capacity(original_bytes_len);
    std::ptr::copy(address as _,original_bytes.as_mut_ptr(),original_bytes_len);
    original_bytes.set_len(original_bytes_len);

    #[cfg(debug_assertions)]
    {
        println!("original bytes: {:x?}",original_bytes);
    }

    let mut patch:Vec<u8> = vec![0xCC;original_bytes_len];
    let jmp:Vec<u8> = vec![0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];

    let myfunction_addr_bytes = usize::to_le_bytes(myfunction);
    if myfunction_addr_bytes.len() != 8 {
        #[cfg(debug_assertions)]
        {
            panic!("address not 8 bytes");
        }
    }

    std::ptr::copy(jmp.as_ptr(),patch.as_mut_ptr(),6);
    std::ptr::copy(myfunction_addr_bytes.as_ptr(), patch.as_mut_ptr().offset(6), 8);

    #[cfg(debug_assertions)]
    {
        println!("new assebly: {:x?}",patch);
    }

    std::ptr::copy(patch.as_ptr(), address as _, original_bytes_len);

    VirtualProtect(address as _, original_bytes_len, original_protection,std::ptr::null_mut());

    #[cfg(debug_assertions)]
    {
        let mut new_bytes:Vec<u8> = Vec::with_capacity(original_bytes_len);
        std::ptr::copy(address as _,new_bytes.as_mut_ptr(),original_bytes_len);
        new_bytes.set_len(original_bytes_len);
        println!("new bytes: {:x?}",new_bytes);
    }

    create_gateway(address, original_bytes)
    }
}


pub fn create_gateway(original_address:usize,original_bytes:Vec<u8>) -> usize {
    unsafe {
        let gateway_size = original_bytes.len()+14;

        let addy = VirtualAlloc(std::ptr::null_mut(), gateway_size, MEM_COMMIT|MEM_RESERVE,PAGE_EXECUTE_READWRITE) as usize;
        
        #[cfg(debug_assertions)] {
            println!("gateway address: {:x}",addy);
        }

        let mut gateway:Vec<u8> = Vec::with_capacity(gateway_size);
        gateway.set_len(gateway_size);

        let jmp = vec![0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];
        std::ptr::copy(original_bytes.as_ptr(),gateway.as_mut_ptr(),original_bytes.len());
        std::ptr::copy(jmp.as_ptr(),gateway.as_mut_ptr().offset(original_bytes.len() as isize),6);

        let original_address_bytes = usize::to_le_bytes(original_address+original_bytes.len());
        if original_address_bytes.len() != 8 {
            #[cfg(debug_assertions)]
            {
                panic!("address not 8 bytes")
            }
        }

        std::ptr::copy(original_address_bytes.as_ptr(),gateway.as_mut_ptr().offset(original_bytes.len() as isize+6),8);

        std::ptr::copy(gateway.as_ptr(),addy as _,gateway.len());


        let mut gateway_bytes:Vec<u8> = Vec::with_capacity(gateway_size);
        std::ptr::copy(addy as _,gateway_bytes.as_mut_ptr(),gateway_size);
        gateway_bytes.set_len(gateway_size);

        #[cfg(debug_assertions)]
        {
            println!("gateway bytes: {:x?}",gateway_bytes);
        }

        VirtualProtect(addy as _, gateway_size, PAGE_EXECUTE_READ,std::ptr::null_mut());
        addy as usize
    }
}