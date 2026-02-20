use reqwest::Client;
use serde::Deserialize;
use serde_with::DurationSeconds;
use serde_with::serde_as;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use url::Url;

use crate::error::MyError;

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct Checks {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub timeout: Duration,
    pub fails: u32,
    pub preflights: Vec<Url>,
    pub shutdowns: Vec<Url>,
}

impl Checks {
    pub async fn preflight(&self, client: &Client) -> Result<u32, MyError> {
        let mut fails = self.fails;
        for preflight in self.preflights.iter() {
            info!("Checking preflight: {}", preflight);
            while fails > 0 && client.get(preflight.clone()).send().await.is_err() {
                info!(
                    "Failed preflight: {} retrying in {} secs (fail count {}/{})",
                    preflight,
                    self.timeout.as_secs(),
                    fails,
                    self.fails
                );
                sleep(self.timeout).await;
                fails -= 1;
            }
        }
        if fails > 0 {
            info!("Preflight success, {} retries remaining", fails);
            Ok(fails)
        } else {
            error!("Preflight FAIL");
            Err(MyError::PreflightCheck)
        }
    }
    pub async fn shutdown(&self, client: &Client) -> Result<u32, MyError> {
        let mut fails = self.fails;
        for shutdown in self.shutdowns.iter() {
            info!("Checking shutdown: {}", shutdown);
            while fails > 0 && client.get(shutdown.clone()).send().await.is_err() {
                info!(
                    "Failed shutdown: {} retrying in {} secs (fail count {}/{})",
                    shutdown,
                    self.timeout.as_secs(),
                    fails,
                    self.fails
                );
                sleep(self.timeout).await;
                fails -= 1;
            }
        }
        if fails > 0 {
            info!("Shutdown success, {} retries remaining", fails);
            Ok(fails)
        } else {
            error!("Shutdown FAIL");
            Err(MyError::ShutdownCheck)
        }
    }
}
