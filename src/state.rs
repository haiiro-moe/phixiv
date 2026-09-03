use reqwest::Client;
use std::env;

#[derive(Clone)]
pub struct PhixivState {
    pub client: Client,
}

impl PhixivState {
    pub async fn login() -> anyhow::Result<Self> {
        let verbose = env::var("TRACE_CLIENT_NETWORK")
            .unwrap_or_else(|_| String::from("false"))
            == "true";

        let client = Client::builder()
            .connection_verbose(verbose)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self { client })
    }
}
