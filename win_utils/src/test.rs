#[cfg(test)]
mod drive_tests {
    use crate::{build_device_to_dos_map, build_dos_to_device_map, get_drive_mappings};

    #[test]
    fn drive_mappings() {
        let x = unsafe { get_drive_mappings() };
        println!("{:?}",x);
        
        let y = build_device_to_dos_map(x.clone());
        println!("{:?}",y);

        let z = build_dos_to_device_map(x);
        println!("{:?}",z)
    }
}