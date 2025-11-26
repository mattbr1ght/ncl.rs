#[cfg(test)]
mod tests {
    use super::super::templates::{Template};
    use std::path::PathBuf;

    #[test]
    fn universal_base_detection_works() {
        let mut tmpl = Template::default();
        tmpl.name = "Universal Base".to_string();
        tmpl.path = PathBuf::from("/tmp/universal-base");

        assert!(tmpl.is_universal_base());
    }
}


