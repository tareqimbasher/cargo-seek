use lazy_static::lazy_static;
use reqwest::Client;
use std::sync::Arc;

lazy_static! {
    pub static ref CLIENT: Arc<Client> = Arc::new(
        Client::builder()
            .user_agent("cargo-seek (https://github.com/tareqimbasher/cargo-seek")
            .build()
            .unwrap()
    );
}
