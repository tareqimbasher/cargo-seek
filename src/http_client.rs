use crate::components::home::SearchResults;
use lazy_static::lazy_static;
use reqwest::{StatusCode, Url};
use std::sync::Arc;

lazy_static! {
    pub static ref INSTANCE: Arc<HttpClient> = Arc::new(HttpClient::new());
}

pub struct HttpClient {
    client: reqwest::Client,
    crates_base_url: Url,
    rate_limit: std::time::Duration,
    last_request_time: Arc<tokio::sync::Mutex<Option<tokio::time::Instant>>>,
}

impl HttpClient {
    fn new() -> Self {
        HttpClient {
            client: reqwest::Client::builder()
                .user_agent("crate-seek (github:tareqimbasher/crate-seek")
                .build()
                .unwrap(),
            crates_base_url: Url::parse("https://crates.io/api/v1/").unwrap(),
            rate_limit: std::time::Duration::from_secs(1),
            last_request_time: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn get(&self, url: &Url) -> (String, StatusCode) {
        let mut lock = self.last_request_time.clone().lock_owned().await;

        if let Some(last_request_time) = lock.take() {
            if last_request_time.elapsed() < self.rate_limit {
                tokio::time::sleep(self.rate_limit - last_request_time.elapsed()).await;
            }
        }

        let time = tokio::time::Instant::now();
        let res = self.client.get(url.clone()).send().await.unwrap();

        let status_code = res.status();
        let content = res.text().await.unwrap();

        // Free up the lock
        *lock = Some(time);

        (content.to_string(), status_code)
    }

    pub async fn search(&self, term: String, page: u32) -> SearchResults {
        let mut url = self.crates_base_url.join("crates").unwrap();

        url.query_pairs_mut()
            .append_pair("q", term.as_str())
            .append_pair("page", page.to_string().as_str())
            .append_pair("per_page", 100.to_string().as_str());

        let (text, _) = self.get(&url).await;

        // TODO if deserialization fails, log it
        serde_json::from_str::<SearchResults>(&text).unwrap_or_default()
    }

    pub async fn get_repo_readme(&self, repo_url_str: String) -> Option<String> {
        let repo_url = Url::parse(repo_url_str.as_str()).unwrap();

        let domain = repo_url.domain();
        if domain.is_none() || domain.unwrap() != "github.com" {
            return None;
        }

        let mut segments = repo_url.path_segments().unwrap();
        let repo_owner = segments.next();
        let repo_name = segments.next();

        if repo_owner.is_none() || repo_name.is_none() {
            return None;
        }

        for branch_name in ["main", "master"] {
            let url = Url::parse(
                format!(
                    "https://raw.githubusercontent.com/{}/{}/refs/heads/{}/README.md",
                    repo_owner.unwrap(),
                    repo_name.unwrap(),
                    branch_name
                )
                .as_str(),
            );

            if url.is_err() {
                return None;
            }

            let (text, status) = self.get(&url.unwrap()).await;

            if status.is_success() {
                return Some(text);
            }
        }

        None
    }
}
