#[cfg(test)]

mod desktop_tests {
    use vnc_module::{desktop::disable_secure_desktop, *};

    #[test]
    fn disable_secure_desktop_test() {
        unsafe { disable_secure_desktop()};
    }
}