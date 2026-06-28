use crate::{BlobRecord, BlobStore};
use agent_os_sys::{AgentOsError, AgentOsResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalBlobStore {
    root: PathBuf,
}

impl LocalBlobStore {
    pub fn new(root: impl Into<PathBuf>) -> AgentOsResult<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("sha256")).map_err(io_error)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        self.root.join("sha256").join(hash)
    }

    fn hash(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl BlobStore for LocalBlobStore {
    fn put_blob(&self, bytes: &[u8]) -> AgentOsResult<BlobRecord> {
        let content_hash = Self::hash(bytes);
        let path = self.blob_path(&content_hash);
        if !path.exists() {
            fs::write(&path, bytes).map_err(io_error)?;
        }
        Ok(BlobRecord {
            blob_ref: format!("sha256:{content_hash}"),
            content_hash,
            byte_len: bytes.len(),
        })
    }

    fn get_blob(&self, blob_ref: &str) -> AgentOsResult<Vec<u8>> {
        let hash = blob_ref
            .strip_prefix("sha256:")
            .ok_or_else(|| AgentOsError::Validation("unsupported blob ref scheme".to_string()))?;
        fs::read(self.blob_path(hash)).map_err(io_error)
    }

    fn has_blob(&self, blob_ref: &str) -> AgentOsResult<bool> {
        let hash = blob_ref
            .strip_prefix("sha256:")
            .ok_or_else(|| AgentOsError::Validation("unsupported blob ref scheme".to_string()))?;
        Ok(self.blob_path(hash).exists())
    }
}

fn io_error(error: std::io::Error) -> AgentOsError {
    AgentOsError::Validation(format!("blob store io error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_blob_store_is_hash_addressed() {
        let root = std::env::temp_dir().join(format!("agent-os-blob-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = LocalBlobStore::new(&root).unwrap();
        let record = store.put_blob(b"hello").unwrap();
        assert!(record.blob_ref.starts_with("sha256:"));
        assert!(store.has_blob(&record.blob_ref).unwrap());
        assert_eq!(store.get_blob(&record.blob_ref).unwrap(), b"hello");
        let duplicate = store.put_blob(b"hello").unwrap();
        assert_eq!(duplicate.content_hash, record.content_hash);
        let _ = std::fs::remove_dir_all(root);
    }
}
