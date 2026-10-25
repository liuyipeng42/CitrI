use atomic_write_file::AtomicWriteFile;
use std::{fs, io::Write, path::Path};

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let write = || -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = AtomicWriteFile::open(path)?;
        file.write_all(bytes)?;
        file.commit()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    };
    write().map_err(|error| format!("无法写入文件：{error}"))
}
