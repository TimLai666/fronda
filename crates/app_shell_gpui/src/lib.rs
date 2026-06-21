pub const APP_NAME: &str = "Fronda";
pub const SHELL_HEADLINE: &str = "Rust rewrite scaffold";
pub const SHELL_STATUS: &str = "Palmier Pro compatibility baseline active";

pub fn launch_status_lines() -> [&'static str; 3] {
    [APP_NAME, SHELL_HEADLINE, SHELL_STATUS]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_status_lines_begin_with_product_name() {
        let lines = launch_status_lines();
        assert_eq!(lines[0], APP_NAME);
        assert_eq!(lines[1], SHELL_HEADLINE);
        assert_eq!(lines[2], SHELL_STATUS);
    }
}
