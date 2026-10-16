//! Private local rules, separate from portable appearance profiles.
use super::{MAX_BYTES, Rules};
use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::PathBuf,
};

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn system() -> Result<Self, String> {
        let dirs =
            directories::ProjectDirs::from("", "", "PTTStella").ok_or("無法找出名單設定目錄")?;
        Ok(Self::at(dirs.config_dir().join("filters.json")))
    }
    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("無法讀取名單：{e}")),
        };
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("無法讀取名單：{e}"))?;
        if bytes.len() > MAX_BYTES {
            return Err("名單超過 256 KiB；原檔保留".into());
        }
        Ok(Some(bytes))
    }
    pub fn load(&self) -> Result<Rules, String> {
        self.read()?
            .map_or_else(|| Ok(Rules::default()), |b| Rules::decode(&b))
    }
    pub fn save(&self, rules: Rules) -> Result<Rules, String> {
        rules.validate().map_err(str::to_owned)?;
        let bytes = serde_json::to_vec_pretty(&rules).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("名單超過 256 KiB".into());
        }
        let previous = self.read()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("無法建立名單目錄：{e}"))?;
        }
        if let Some(previous) = previous {
            crate::storage::replace(&self.path.with_extension("json.bak"), &previous)?;
        }
        crate::storage::replace(&self.path, &bytes)?;
        Ok(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_reload_with_backup_and_failures_preserve_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("filters.json");
        let store = Store::at(path.clone());
        let rules = store
            .load()
            .unwrap()
            .edit("Alice", "自製測試", false)
            .unwrap();
        store.save(rules.clone()).unwrap();
        let old = std::fs::read(&path).unwrap();
        let next = rules.edit("alice", "改註", false).unwrap();
        store.save(next.clone()).unwrap();
        assert_eq!(store.load().unwrap(), next);
        assert_eq!(std::fs::read(path.with_extension("json.bak")).unwrap(), old);
        let current = std::fs::read(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(store.save(Rules::default()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), current);
    }
    #[test]
    fn invalid_and_oversized_files_are_never_silently_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("filters.json");
        let store = Store::at(path.clone());
        std::fs::write(&path, b"broken").unwrap();
        assert!(store.load().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        store.save(Rules::default()).unwrap();
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            b"broken"
        );
        std::fs::write(&path, vec![b'x'; MAX_BYTES + 1]).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(Rules::default()).is_err());
        assert_eq!(
            std::fs::metadata(path).unwrap().len(),
            (MAX_BYTES + 1) as u64
        );
    }
}
