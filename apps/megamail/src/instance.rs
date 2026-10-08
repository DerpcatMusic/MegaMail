use std::{
    fs::{File, OpenOptions},
    io,
    path::Path,
};

/// Keep the returned file alive while workers run: a second process must not
/// sync or retry the same Outbox concurrently.
pub fn acquire(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_process_lock_is_rejected_and_crash_free_release_is_automatic() {
        let path =
            std::env::temp_dir().join(format!("megamail-instance-{}.lock", std::process::id()));
        let first = acquire(&path).unwrap();
        assert_eq!(
            acquire(&path).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(first);
        drop(acquire(&path).unwrap());
        std::fs::remove_file(path).unwrap();
    }
}
