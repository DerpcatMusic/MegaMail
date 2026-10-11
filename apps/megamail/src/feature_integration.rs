//! Native ingress and action policy. No provider/profile operations in this seam.
use std::ffi::OsString;
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Startup {
    pub demo: bool,
    pub uri: Option<String>,
}
pub(crate) fn startup(arguments: Vec<OsString>) -> Result<Startup, &'static str> {
    match arguments.as_slice() {
        [] => Ok(Startup {
            demo: false,
            uri: None,
        }),
        [arg] if arg == "--demo" => Ok(Startup {
            demo: true,
            uri: None,
        }),
        [arg] => {
            let uri = arg.to_str().ok_or("Startup input must be exact UTF-8.")?;
            if uri.len() > 64 * 1024
                || !uri
                    .get(..7)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("mailto:"))
            {
                return Err("Use no argument, --demo, or one bounded mailto URI.");
            }
            // Actual parse is exactly once, at instance::start_activation.
            Ok(Startup {
                demo: false,
                uri: Some(uri.to_owned()),
            })
        }
        _ => Err("Only one mailto URI is supported; unknown flags/extra arguments are rejected."),
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn ingress_is_bounded_and_never_redecodes() {
        assert_eq!(
            super::startup(vec!["mailto:a@example.test?body=%252B+".into()])
                .unwrap()
                .uri
                .unwrap(),
            "mailto:a@example.test?body=%252B+"
        );
        assert!(super::startup(vec!["--demo".into(), "mailto:x".into()]).is_err());
        assert!(super::startup(vec!["--thunderbird-host".into()]).is_err());
    }
    #[test]
    fn ingress_rejects_non_utf8_without_lossy_conversion() {
        use std::os::unix::ffi::OsStringExt;
        assert!(super::startup(vec![std::ffi::OsString::from_vec(vec![0xff])]).is_err());
    }
}
