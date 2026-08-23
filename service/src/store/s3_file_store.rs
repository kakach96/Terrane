//! S3 object storage backend for file-based data sources.
//!
//! Implements the [`FileStore`] trait against any S3-compatible service
//! (AWS S3, MinIO, etc.) via the `rust-s3` crate. Connection details
//! (endpoint / region / bucket / access & secret keys) come from the data
//! source's `DataSourceConnection` (`s3_*` fields), so the backend is chosen
//! per data source through `file_storage_type`.
//!
//! `local_path` always returns `None` here: callers must stream bytes through
//! [`FileStore::get`] or materialize the object to a local temp file.

use crate::models::DataSourceConnection;
use crate::store::{FileStore, StoreEntry, StoreError};
use async_trait::async_trait;
use s3::{creds::Credentials, region::Region, Bucket};
use std::path::PathBuf;

/// S3 object storage file store.
pub struct S3FileStore {
    bucket: Bucket,
}

impl S3FileStore {
    /// Build an S3 file store from a data source connection.
    ///
    /// Requires `file_storage_type = "s3"` and `s3_bucket`; `s3_endpoint`
    /// selects a custom endpoint (MinIO, etc.), otherwise the AWS region
    /// endpoint is used. Blank credentials fall back to anonymous access.
    pub fn from_connection(conn: &DataSourceConnection) -> Result<Self, StoreError> {
        // 解析 ${ENV_VAR} 引用的凭据 (K8s Secrets 注入), 仅本地副本, 不落库
        let mut resolved = conn.clone();
        crate::utils::secrets::resolve_connection_secrets(&mut resolved);
        let conn = &resolved;

        let bucket_name = conn
            .s3_bucket
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| StoreError::Other("S3 data source requires s3_bucket".to_string()))?;

        let region_name = conn
            .s3_region
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("us-east-1");

        let region = match conn.s3_endpoint.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(endpoint) => Region::Custom {
                region: region_name.to_string(),
                endpoint: endpoint.to_string(),
            },
            None => region_name
                .parse()
                .map_err(|_| StoreError::Other(format!("Invalid S3 region: {}", region_name)))?,
        };

        let credentials = {
            let ak = conn
                .s3_access_key
                .as_deref()
                .filter(|s| !s.trim().is_empty());
            let sk = conn
                .s3_secret_key
                .as_deref()
                .filter(|s| !s.trim().is_empty());
            match (ak, sk) {
                (None, None) => Credentials::anonymous()
                    .map_err(|e| StoreError::Other(format!("S3 credentials error: {}", e)))?,
                _ => Credentials::new(ak, sk, None, None, None)
                    .map_err(|e| StoreError::Other(format!("S3 credentials error: {}", e)))?,
            }
        };

        let bucket = Bucket::new(bucket_name, region, credentials)
            .map_err(|e| StoreError::Other(format!("S3 bucket init error: {}", e)))?
            .with_path_style();

        Ok(Self { bucket })
    }

    /// List the bucket entries under a prefix (folders + objects), mirroring a
    /// directory listing. Used by the S3 browse endpoint.
    pub async fn browse(&self, prefix: &str) -> Result<Vec<StoreEntry>, StoreError> {
        let pages = self
            .bucket
            .list(prefix.to_string(), Some("/".to_string()))
            .await
            .map_err(|e| StoreError::Other(format!("S3 list error: {}", e)))?;

        let mut entries: Vec<StoreEntry> = Vec::new();
        for page in pages {
            if let Some(common) = page.common_prefixes {
                for cp in common {
                    // 保留尾部 '/' 作为导航前缀, 避免用无斜杠前缀再次列出自身
                    let p = cp.prefix.clone();
                    let name = p
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(&p)
                        .to_string();
                    entries.push(StoreEntry {
                        name,
                        path: p,
                        is_dir: true,
                        size: 0,
                    });
                }
            }
            for obj in page.contents {
                // Skip folder-marker objects (keys ending with '/').
                if obj.key.ends_with('/') {
                    continue;
                }
                let name = obj.key.rsplit('/').next().unwrap_or(&obj.key).to_string();
                entries.push(StoreEntry {
                    name,
                    path: obj.key,
                    is_dir: false,
                    size: obj.size,
                });
            }
        }

        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        Ok(entries)
    }

    /// List object keys under a prefix (used to locate multi-file sidecars
    /// such as the .dbf / .shx / .prj of a Shapefile).
    pub async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let pages = self
            .bucket
            .list(prefix.to_string(), None)
            .await
            .map_err(|e| StoreError::Other(format!("S3 list error: {}", e)))?;
        let mut keys = Vec::new();
        for page in pages {
            for obj in page.contents {
                keys.push(obj.key);
            }
        }
        keys.sort();
        Ok(keys)
    }

    /// List all object keys in the bucket.
    pub async fn list_all_keys(&self) -> Result<Vec<String>, StoreError> {
        self.list_prefix("").await
    }
}

#[async_trait]
impl FileStore for S3FileStore {
    async fn put(&self, key: &str, data: &[u8]) -> Result<(), StoreError> {
        self.bucket
            .put_object(key, data)
            .await
            .map_err(|e| StoreError::Other(format!("S3 put error: {}", e)))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        match self.bucket.get_object(key).await {
            Ok(resp) => Ok(Some(resp.as_slice().to_vec())),
            Err(s3::error::S3Error::HttpFailWithBody(404, _)) => Ok(None),
            Err(e) => Err(StoreError::Other(format!("S3 get error: {}", e))),
        }
    }

    fn local_path(&self, _key: &str) -> Option<PathBuf> {
        None
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| StoreError::Other(format!("S3 delete error: {}", e)))?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, StoreError> {
        self.list_all_keys().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_connection_missing_bucket() {
        let conn = DataSourceConnection::default();
        assert!(
            S3FileStore::from_connection(&conn).is_err(),
            "缺少 s3_bucket 时应返回错误"
        );
    }

    #[test]
    fn test_browse_relative_name_extraction() {
        // Sanity check of the name/path mapping helper logic used by browse.
        let key = "folder/sub/file.geojson";
        let name = key.rsplit('/').next().unwrap();
        assert_eq!(name, "file.geojson");
        let prefix = "folder/sub";
        let name2 = prefix.rsplit('/').next().unwrap();
        assert_eq!(name2, "sub");
    }
}
