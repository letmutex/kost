use ahash::{AHashMap, AHashSet};
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};

use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::progress::TaskTracker;
use crate::repo::RepositoryRegistry;

#[derive(Debug)]
pub enum FetchError {
    NotFound(u16),
    Http(u16),
    Connection(String),
    Other(String),
}

/// HTTP downloader for remote Maven artifacts with bounded concurrency.
#[derive(Clone)]
pub struct Downloader {
    client: Client,
    concurrency: usize,
    semaphore: Arc<Semaphore>,
    unreachable_repos: Arc<RwLock<AHashSet<String>>>,
    connection_failures: Arc<RwLock<AHashMap<String, usize>>>,
}

impl Default for Downloader {
    fn default() -> Self {
        let concurrency = Self::concurrency_from_env().unwrap_or(Self::DEFAULT_CONCURRENCY);
        Self::new(concurrency)
    }
}

impl Downloader {
    pub const DEFAULT_CONCURRENCY: usize = 32;
    pub const MAX_CONCURRENCY: usize = 256;

    pub fn parse_concurrency(val: &str) -> Option<usize> {
        val.trim()
            .parse::<usize>()
            .ok()
            .map(|c| c.clamp(1, Self::MAX_CONCURRENCY))
    }

    pub fn resolve_concurrency(env_val: Option<&str>) -> usize {
        env_val
            .and_then(Self::parse_concurrency)
            .unwrap_or(Self::DEFAULT_CONCURRENCY)
    }

    pub fn concurrency_from_env() -> Option<usize> {
        let val = std::env::var("KOST_CONCURRENT_DOWNLOADS")
            .or_else(|_| std::env::var("kost_concurrent_downloads"))
            .ok()?;
        Self::parse_concurrency(&val)
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency
    }

    pub fn new(concurrency: usize) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(3))
            .user_agent("kost-dep-fetcher/0.1.0")
            .build()
            .unwrap_or_else(|_| Client::new());

        let bounded_concurrency = concurrency.clamp(1, Self::MAX_CONCURRENCY);
        Self {
            client,
            concurrency: bounded_concurrency,
            semaphore: Arc::new(Semaphore::new(bounded_concurrency)),
            unreachable_repos: Arc::new(RwLock::new(AHashSet::default())),
            connection_failures: Arc::new(RwLock::new(AHashMap::default())),
        }
    }

    pub async fn is_repo_unreachable(&self, url: &str) -> bool {
        self.unreachable_repos.read().await.contains(url)
    }

    pub async fn record_connection_success(&self, url: &str) {
        if self.connection_failures.read().await.contains_key(url) {
            self.connection_failures.write().await.remove(url);
        }
    }

    pub async fn record_connection_failure(&self, url: &str) {
        let mut failures = self.connection_failures.write().await;
        let count = failures.entry(url.to_string()).or_insert(0);
        *count += 1;
        if *count >= 2 {
            self.unreachable_repos.write().await.insert(url.to_string());
        }
    }

    pub(crate) async fn record_probe_error(&self, url: &str, err: &FetchError) {
        match err {
            FetchError::NotFound(_) => self.record_connection_success(url).await,
            FetchError::Connection(_) => self.record_connection_failure(url).await,
            _ => {}
        }
    }

    async fn fetch_bytes_streamed(
        &self,
        url: &str,
        tracker: &mut Option<TaskTracker>,
    ) -> Result<bytes::Bytes, FetchError> {
        if let Some(rest) = url.strip_prefix("file://") {
            let path_str = if cfg!(windows) && rest.starts_with('/') {
                &rest[1..]
            } else {
                rest
            };
            let data = std::fs::read(path_str).map_err(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    FetchError::NotFound(404)
                } else {
                    FetchError::Other(common::concat_str!("File read error: ", &err.to_string()))
                }
            })?;
            if let Some(active_tracker) = tracker {
                active_tracker.set_total_bytes(data.len() as u64);
                active_tracker.update(data.len() as u64);
            }
            return Ok(bytes::Bytes::from(data));
        }

        let mut response = self.client.get(url).send().await.map_err(|err| {
            if err.is_connect() || err.is_timeout() {
                FetchError::Connection(err.to_string())
            } else {
                FetchError::Other(err.to_string())
            }
        })?;

        if !response.status().is_success() {
            let code = response.status().as_u16();
            return Err(if response.status().is_server_error() || code == 408 {
                FetchError::Http(code)
            } else {
                FetchError::NotFound(code)
            });
        }

        if let Some(content_length) = response.content_length()
            && let Some(active_tracker) = tracker
        {
            active_tracker.set_total_bytes(content_length);
        }

        let mut buf = Vec::with_capacity(response.content_length().unwrap_or(32 * 1024) as usize);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|err| FetchError::Other(err.to_string()))?
        {
            if let Some(active_tracker) = tracker {
                active_tracker.update(chunk.len() as u64);
            }
            buf.extend_from_slice(&chunk);
        }

        Ok(bytes::Bytes::from(buf))
    }

    async fn fetch_bytes_internal(&self, url: &str) -> Result<bytes::Bytes, FetchError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| FetchError::Other(e.to_string()))?;
        self.fetch_bytes_streamed(url, &mut None).await
    }

    pub(crate) async fn fetch_text_internal(&self, url: &str) -> Result<String, FetchError> {
        let bytes = self.fetch_bytes_internal(url).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub async fn fetch_bytes(&self, url: &str) -> Result<bytes::Bytes, String> {
        self.fetch_bytes_internal(url).await.map_err(|e| match e {
            FetchError::NotFound(c) | FetchError::Http(c) => {
                common::concat_str!("HTTP error: ", &c.to_string())
            }
            FetchError::Connection(msg) | FetchError::Other(msg) => msg,
        })
    }

    pub async fn fetch_text(&self, url: &str) -> Result<String, String> {
        let bytes = self.fetch_bytes(url).await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Resolve metadata for a dependency coordinate across repositories in online mode (`offline: false`).
    pub async fn resolve_metadata(
        &self,
        registry: &RepositoryRegistry,
        coord: &DependencyCoordinate,
        cache: &GradleCache,
    ) -> Result<Vec<DependencyCoordinate>, String> {
        crate::metadata::resolve_metadata(self, registry, coord, cache, false).await
    }

    /// Download an artifact (binary or sources) across repositories to the Gradle cache.
    pub async fn download_artifact(
        &self,
        registry: &RepositoryRegistry,
        coord: &DependencyCoordinate,
        is_sources: bool,
        cache: &GradleCache,
    ) -> Result<PathBuf, String> {
        self.download_artifact_with_tracker(registry, coord, is_sources, cache, None)
            .await
    }

    /// Download an artifact with optional progress tracking.
    pub async fn download_artifact_with_tracker(
        &self,
        registry: &RepositoryRegistry,
        coord: &DependencyCoordinate,
        is_sources: bool,
        cache: &GradleCache,
        mut tracker: Option<TaskTracker>,
    ) -> Result<PathBuf, String> {
        if let Some(existing) = cache.find_artifact(coord, is_sources) {
            if let Some(t) = tracker {
                t.finish();
            }
            return Ok(existing);
        }

        let _permit = self.semaphore.acquire().await.map_err(|e| e.to_string())?;

        if let Some(t) = tracker.as_mut() {
            t.start();
        }

        let mut candidates = Vec::with_capacity(4);
        for cand_name in coord.kmp_probe_candidates() {
            let mut target = coord.clone();
            target.name = smol_str::SmolStr::new(&cand_name);
            candidates.push((target.clone(), is_sources));
            if !is_sources && target.extension == "jar" {
                let mut aar = target;
                aar.extension = smol_str::SmolStr::new("aar");
                candidates.push((aar, false));
            }
        }

        for (target, sources) in candidates {
            let file_name = target.artifact_file_name(sources);
            if let Some(path) = self
                .try_download_candidate(registry, &target, &file_name, sources, cache, &mut tracker)
                .await?
            {
                if let Some(t) = tracker {
                    t.finish();
                }
                return Ok(path);
            }
        }

        let err_msg = common::concat_str!(
            "Failed to download '",
            &coord.artifact_file_name(is_sources),
            "' from any configured repository."
        );
        if let Some(t) = tracker {
            t.fail(&err_msg);
        }
        Err(err_msg)
    }

    async fn try_download_candidate(
        &self,
        registry: &RepositoryRegistry,
        coord: &DependencyCoordinate,
        file_name: &str,
        is_sources: bool,
        cache: &GradleCache,
        tracker: &mut Option<TaskTracker>,
    ) -> Result<Option<PathBuf>, String> {
        for repo in &registry.repositories {
            if self.is_repo_unreachable(&repo.url).await {
                continue;
            }
            let url = repo.artifact_url(coord, is_sources);
            match self.fetch_bytes_streamed(&url, tracker).await {
                Ok(bytes) => {
                    self.record_connection_success(&repo.url).await;
                    return cache.store_file(coord, file_name, &bytes).map(Some);
                }
                Err(err) => {
                    if let Some(t) = tracker {
                        t.reset();
                    }
                    self.record_probe_error(&repo.url, &err).await;
                }
            }
        }
        Ok(None)
    }
}
