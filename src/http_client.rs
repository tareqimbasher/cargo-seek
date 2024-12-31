// use lazy_static::lazy_static;
// use reqwest::{StatusCode, Url};
// use std::sync::{
//     atomic::{AtomicUsize, Ordering},
//     Arc,
// };
// use tokio::sync::Mutex;
//
// use crate::components::home::sort_dropdown::Sort;
// use crate::components::home::search_results::SearchResults;
// use crate::errors::{AppError, AppResult};
//
// lazy_static! {
//     pub static ref INSTANCE: Arc<HttpClient> = Arc::new(HttpClient::new());
//     static ref CLIENT: Arc<reqwest::Client> = Arc::new(
//         reqwest::Client::builder()
//             .user_agent("seekr (github:tareqimbasher/seekr")
//             .build()
//             .unwrap()
//     );
// }
//
// pub struct HttpClient {
//     rate_limit: std::time::Duration,
//     last_request_time: Arc<Mutex<Option<tokio::time::Instant>>>,
//     active_requests: Arc<AtomicUsize>,
//
//     crates_base_url: Url,
// }
//
// impl Default for HttpClient {
//     fn default() -> Self {
//         HttpClient {
//             rate_limit: std::time::Duration::from_secs(1),
//             last_request_time: Arc::new(Mutex::new(None)),
//             active_requests: Arc::new(AtomicUsize::new(0)),
//
//             crates_base_url: Url::parse("https://crates.io/api/v1/").unwrap(),
//         }
//     }
// }
//
// impl HttpClient {
//     pub fn new() -> Self {
//         Default::default()
//     }
//
//     async fn get(&self, url: &Url) -> AppResult<(String, StatusCode)> {
//         match CLIENT.get(url.clone()).send().await {
//             Ok(res) => {
//                 let status_code = res.status();
//                 match res.text().await {
//                     Ok(content) => Ok((content, status_code)),
//                     Err(err) => Err(AppError::from(err)),
//                 }
//             }
//             Err(err) => Err(AppError::from(err)),
//         }
//     }
//
//     /// Rate-limited request
//     pub async fn rate_limited_get(&self, url: &Url) -> AppResult<(String, StatusCode)> {
//         let mut lock = self.last_request_time.lock().await;
//
//         if let Some(last_request_time) = *lock {
//             if last_request_time.elapsed() < self.rate_limit {
//                 tokio::time::sleep(self.rate_limit - last_request_time.elapsed()).await;
//             }
//         }
//
//         self.active_requests.fetch_add(1, Ordering::SeqCst);
//
//         let time = tokio::time::Instant::now();
//         let result = self.get(url).await;
//
//         self.active_requests.fetch_sub(1, Ordering::SeqCst);
//
//         // Free up the lock
//         *lock = Some(time);
//
//         result
//     }
//
//     #[allow(dead_code)]
//     /// Non-rate-limited request
//     pub async fn non_rate_limited_get(&self, url: &Url) -> AppResult<(String, StatusCode)> {
//         self.active_requests.fetch_add(1, Ordering::SeqCst);
//
//         self.active_requests.fetch_add(1, Ordering::SeqCst);
//
//         let result = self.get(url).await;
//
//         self.active_requests.fetch_sub(1, Ordering::SeqCst);
//         self.active_requests.fetch_sub(1, Ordering::SeqCst);
//
//         result
//     }
//
//     /// Checks if the client is currently working (non-blocking)
//     pub fn is_working(&self) -> bool {
//         self.active_requests.load(Ordering::SeqCst) > 0
//     }
//
//     pub async fn search(
//         &self,
//         term: String,
//         sort: Sort,
//         per_page: usize,
//         page: usize,
//     ) -> AppResult<SearchResults> {
//         let mut url = self
//             .crates_base_url
//             .join("crates")
//             .map_err(|err| AppError::Url(format!("{}", err)))?;
//
//         url.query_pairs_mut()
//             .append_pair("q", term.as_str())
//             .append_pair("sort", sort.to_str())
//             .append_pair("page", page.to_string().as_str())
//             .append_pair("per_page", per_page.to_string().as_str());
//
//         let (text, status) = self.rate_limited_get(&url).await?;
//
//         if !status.is_success() {
//             return Err(AppError::ResponseUnsuccessful(u16::from(status), text));
//         }
//
//         let mut results = serde_json::from_str::<SearchResults>(&text)?;
//
//         // results.set_current_page(page);
//
//         Ok(results)
//     }
//
//     #[allow(dead_code)]
//     pub async fn get_repo_readme(&self, repo_url_str: String) -> AppResult<Option<String>> {
//         let repo_url =
//             Url::parse(repo_url_str.as_str()).map_err(|err| AppError::Url(format!("{}", err)))?;
//
//         let domain = repo_url.domain();
//         if domain.is_none() || domain.unwrap() != "github.com" {
//             return Ok(None);
//         }
//
//         if let Some(mut segments) = repo_url.path_segments() {
//             let repo_owner = segments.next();
//             let repo_name = segments.next();
//
//             if repo_owner.is_none() || repo_name.is_none() {
//                 return Ok(None);
//             }
//
//             for branch_name in ["main", "master"] {
//                 let url = Url::parse(
//                     format!(
//                         "https://raw.githubusercontent.com/{}/{}/refs/heads/{}/README.md",
//                         repo_owner.unwrap(),
//                         repo_name.unwrap(),
//                         branch_name
//                     )
//                     .as_str(),
//                 )
//                 .map_err(|err| AppError::Url(format!("{}", err)))?;
//
//                 let (text, status) = self.non_rate_limited_get(&url).await?;
//
//                 if status.is_success() {
//                     return Ok(Some(text));
//                 }
//             }
//         }
//
//         Ok(None)
//     }
// }
