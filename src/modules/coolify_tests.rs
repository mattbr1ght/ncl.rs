#[cfg(test)]
mod tests {
    use super::super::coolify::ApplicationEnvironmentVariable;

    #[test]
    fn coolify_defaults_constructor_sets_flags() {
        let var = ApplicationEnvironmentVariable::with_coolify_defaults("KEY", "VALUE");

        assert_eq!(var.key, "KEY");
        assert_eq!(var.value, "VALUE");
        assert_eq!(var.is_multiline, Some(true));
        assert_eq!(var.is_preview, Some(true));
        assert_eq!(var.is_literal, Some(true));
        assert_eq!(var.is_shown_once, Some(true));
    }
}


