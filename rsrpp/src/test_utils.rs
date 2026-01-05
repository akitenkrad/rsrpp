use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use strum::Display;

use anyhow::{anyhow, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{error, info};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SamplePaper {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub title: String,
}
impl SamplePaper {
    pub fn dest_path(&self, dir: &Path) -> PathBuf {
        dir.join(&self.filename)
    }
}

/// 内蔵サンプル論文 (URL, ファイル名, タイトル)
#[derive(Copy, Clone, Debug, Display)]
pub enum BuiltinPaper {
    AttentionIsAllYouNeed,
    UnsupervisedDialoguePolicies,
    MemAgent,
    AlgorithmOfThoughts,
    LearningToUseAiForLearning,
    ZepTemporalKnowledgeGraph,
}

impl BuiltinPaper {
    pub const ALL: [BuiltinPaper; 6] = [
        BuiltinPaper::AttentionIsAllYouNeed,
        BuiltinPaper::UnsupervisedDialoguePolicies,
        BuiltinPaper::MemAgent,
        BuiltinPaper::AlgorithmOfThoughts,
        BuiltinPaper::LearningToUseAiForLearning,
        BuiltinPaper::ZepTemporalKnowledgeGraph,
    ];

    pub fn meta(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            BuiltinPaper::AttentionIsAllYouNeed => (
                "https://arxiv.org/pdf/1706.03762",
                "1706.03762.pdf",
                "Attention Is All You Need",
            ),
            BuiltinPaper::UnsupervisedDialoguePolicies => (
                "https://aclanthology.org/2024.emnlp-main.1060.pdf",
                "2024.emnlp-main.1060.pdf",
                "Unsupervised Extraction of Dialogue Policies from Conversations",
            ),
            BuiltinPaper::MemAgent => (
                "https://arxiv.org/pdf/2507.02259",
                "2507.02259.pdf",
                "MemAgent: Reshaping Long-Context LLM with Multi-Conv RL-based Memory Agent",
            ),
            BuiltinPaper::AlgorithmOfThoughts => (
                "https://arxiv.org/pdf/2308.10379",
                "2308.10379.pdf",
                "Algorithm of Thoughts: Enhancing Exploration of Ideas in Large Language Models",
            ),
            BuiltinPaper::LearningToUseAiForLearning => (
                "https://arxiv.org/pdf/2508.13962",
                "2508.13962.pdf",
                "Learning to Use AI for Learning: How Can We Effectively Teach and Measure Prompting Literacy for K–12 Students?",
            ),
            BuiltinPaper::ZepTemporalKnowledgeGraph => (
                "https://arxiv.org/pdf/2501.13956",
                "2501.13956.pdf",
                "Zep: A Temporal Knowledge Graph Architecture for Agent Memory",
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpecEntry {
    url: String,
    filename: String,
    title: String,
}

impl From<(String, String)> for SpecEntry {
    fn from(v: (String, String)) -> Self {
        let (url, filename) = v;
        let title = filename.clone(); // fallback: use filename as title
        Self {
            url,
            filename,
            title,
        }
    }
}
impl From<(String, String, String)> for SpecEntry {
    fn from(v: (String, String, String)) -> Self {
        let (url, filename, title) = v;
        Self {
            url,
            filename,
            title,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestPapers {
    pub papers: Vec<SamplePaper>,
    pub tmp_dir: PathBuf,
}

impl TestPapers {
    pub async fn setup() -> Result<Self> {
        let specs: Vec<(String, String, String)> = BuiltinPaper::ALL
            .iter()
            .map(|p| {
                let (u, f, t) = p.meta();
                (u.to_string(), f.to_string(), t.to_string())
            })
            .collect();
        Self::setup_with(specs).await
    }

    /// specs: Vec of (url, filename, title) OR legacy (url, filename)
    pub async fn setup_with<T>(specs: Vec<T>) -> Result<Self>
    where
        T: Into<SpecEntry>,
    {
        let specs: Vec<SpecEntry> = specs.into_iter().map(|e| e.into()).collect();
        if specs.is_empty() {
            return Err(anyhow!("spec list empty"));
        }
        let mut tmp_dir = std::env::temp_dir();
        tmp_dir.push(format!(
            "rsrpp_test_{}_{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        ));
        fs::create_dir_all(&tmp_dir)?;
        let mut cache_dir = std::env::temp_dir();
        cache_dir.push("rsrpp_test_cache");
        fs::create_dir_all(&cache_dir)?;
        let ttl_secs: u64 = env_parse("RSRPP_TEST_CACHE_TTL_SECONDS", 24 * 3600);
        let timeout_secs: u64 = env_parse("RSRPP_TEST_HTTP_TIMEOUT", 30);
        let max_retries: u32 = env_parse("RSRPP_TEST_HTTP_RETRIES", 3);
        let concurrency: usize = env_parse("RSRPP_TEST_HTTP_CONCURRENCY", 4);
        let client = Client::builder().build()?;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
        let progress = Arc::new(AtomicUsize::new(0));
        let total = specs.len();
        let mut tasks = FuturesUnordered::new();
        for entry in specs.iter() {
            let url = entry.url.clone();
            let filename = entry.filename.clone();
            let tmp_c = tmp_dir.clone();
            let cache_c = cache_dir.clone();
            let client_c = client.clone();
            let progress_c = progress.clone();
            let sem_c = semaphore.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = sem_c.acquire_owned().await.expect("semaphore");
                let res = download_with_cache(
                    &client_c,
                    &url,
                    &filename,
                    &cache_c,
                    &tmp_c,
                    ttl_secs,
                    timeout_secs,
                    max_retries,
                )
                .await;
                if res.is_ok() {
                    let d = progress_c.fetch_add(1, Ordering::SeqCst) + 1;
                    info!("{}/{} {}", d, total, filename);
                }
                (filename, res)
            }));
        }
        let mut map = HashMap::new();
        let mut completed_tasks = 0;
        let mut failed_downloads = Vec::new();

        // Wait for all download tasks to complete
        while let Some(j) = tasks.next().await {
            completed_tasks += 1;
            match j {
                Ok((f, Ok(p))) => {
                    map.insert(f, p);
                }
                Ok((f, Err(e))) => {
                    error!("FAIL {}: {}", f, e);
                    failed_downloads.push(f);
                }
                Err(e) => {
                    error!("JoinErr: {}", e);
                    failed_downloads.push("unknown".to_string());
                }
            }
        }

        // Ensure all tasks have completed
        if completed_tasks != total {
            return Err(anyhow!(
                "Not all download tasks completed: {}/{} completed",
                completed_tasks,
                total
            ));
        }

        // Report failed downloads
        if !failed_downloads.is_empty() {
            return Err(anyhow!(
                "Failed to download {} files: {:?}",
                failed_downloads.len(),
                failed_downloads
            ));
        }

        // Check that all expected files are in the map
        for entry in specs.iter() {
            if !map.contains_key(&entry.filename) {
                return Err(anyhow!("Missing {}", entry.filename));
            }
        }
        // Check the all files have been downloaded
        info!("Verifying downloaded files...");
        for entry in specs.iter() {
            let file_path = tmp_dir.join(&entry.filename);
            if !file_path.exists() {
                return Err(anyhow!(
                    "Downloaded file does not exist: {}",
                    entry.filename
                ));
            }

            // Check if the file is not empty
            let metadata = fs::metadata(&file_path)?;
            if metadata.len() == 0 {
                return Err(anyhow!("Downloaded file is empty: {}", entry.filename));
            }

            // Check if the file is readable
            if let Err(e) = fs::File::open(&file_path) {
                return Err(anyhow!(
                    "Cannot open downloaded file {}: {}",
                    entry.filename,
                    e
                ));
            }

            // Log file information
            info!(
                "✓ {} ({} bytes) - {}",
                entry.filename,
                metadata.len(),
                file_path.display()
            );
        }

        info!(
            "All {} files successfully downloaded and verified",
            specs.len()
        );

        let papers = specs
            .into_iter()
            .map(|e| SamplePaper {
                id: e.filename.clone(),
                url: e.url,
                filename: e.filename,
                title: e.title,
            })
            .collect();
        Ok(Self { papers, tmp_dir })
    }
    pub fn cleanup(&self) -> Result<()> {
        if self.tmp_dir.exists() {
            fs::remove_dir_all(&self.tmp_dir)?;
        }
        Ok(())
    }
    pub fn setup_blocking() -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(Self::setup())
    }

    pub fn get_by_title(&self, paper: BuiltinPaper) -> Option<&SamplePaper> {
        let (_u, filename, _t) = paper.meta();
        self.papers.iter().find(|p| p.filename == filename)
    }
}

pub fn setup_test_papers_blocking() -> Result<TestPapers> {
    TestPapers::setup_blocking()
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

async fn download_with_cache(
    client: &Client,
    url: &str,
    filename: &str,
    cache_dir: &Path,
    tmp_dir: &Path,
    ttl_secs: u64,
    timeout_secs: u64,
    max_retries: u32,
) -> Result<PathBuf> {
    let cache_path = cache_dir.join(filename);
    let dst_path = tmp_dir.join(filename);
    if cache_path.exists() {
        if let Ok(meta) = fs::metadata(&cache_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() <= ttl_secs {
                        fs::copy(&cache_path, &dst_path)?;
                        return Ok(dst_path);
                    }
                }
            }
        }
    }
    let mut last_err = anyhow!("download failed");
    for attempt in 1..=max_retries {
        match timeout(Duration::from_secs(timeout_secs), client.get(url).send()).await {
            Ok(Ok(resp)) => {
                if resp.status().is_success() {
                    let bytes = resp.bytes().await?;
                    let tmp_file = cache_path.with_extension("tmp");
                    fs::write(&tmp_file, &bytes)?;
                    fs::rename(&tmp_file, &cache_path)?;
                    fs::copy(&cache_path, &dst_path)?;
                    return Ok(dst_path);
                } else {
                    last_err = anyhow!("bad status {}", resp.status());
                }
            }
            Ok(Err(e)) => {
                last_err = anyhow!(e);
            }
            Err(_) => {
                last_err = anyhow!("timeout");
            }
        }
        if attempt < max_retries {
            // backoff before next retry
            tokio::time::sleep(Duration::from_millis(130 * attempt as u64)).await;
        }
    }
    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_download() {
        let tp = TestPapers::setup().await.unwrap();
        assert_eq!(tp.papers.len(), 6);
        for p in &tp.papers {
            assert!(p.dest_path(&tp.tmp_dir).exists());
        }
        tp.cleanup().unwrap();
    }
}
