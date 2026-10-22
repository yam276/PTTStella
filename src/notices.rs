//! Portable delivery can export its compiled notices before creating a WebView.
use std::{fs::OpenOptions, io::Write, path::Path};

const EMBEDDED: &str = include_str!(concat!(env!("OUT_DIR"), "/third-party-notices.txt"));

pub fn export(path: &Path) -> std::io::Result<()> {
    write_new(path, EMBEDDED)
}

fn write_new(path: &Path, text: &str) -> std::io::Result<()> {
    if text.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "This build has no embedded notices; see the package's licenses directory",
        ));
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    // The caller owns only the file just created. Preserve any pre-existing file.
    if let Err(error) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        std::fs::remove_file(path)?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_export_preserves_existing_files_and_rejects_missing_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("第三方授權.txt");
        assert!(write_new(&path, "").is_err());
        assert!(!path.exists());
        write_new(&path, "Synthetic license\n合成授權\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Synthetic license\n合成授權\n"
        );
        assert_eq!(
            write_new(&path, "replacement").unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Synthetic license\n合成授權\n"
        );
    }
}
