use sp_runtime::offchain::{http, Duration};
use sp_runtime::sp_std::str;
use sp_runtime::Vec;
use serde::{Deserialize, Serialize};

pub trait PriceProvider {
    fn fetch_price() -> Result<u32, http::Error>;
}

#[derive(Serialize, Deserialize)]
struct CryptoCompareResponse {
    #[serde(rename = "USD")]
    usd: f64,
}

pub struct CryptoCompareProvider;

impl PriceProvider for CryptoCompareProvider {
    fn fetch_price() -> Result<u32, http::Error> {
        // 2 seconds timeout for not hanging the node
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));

        let request = http::Request::get(
            "https://min-api.cryptocompare.com/data/price?fsym=ADA&tsyms=USD",
        );

        let pending = request
            .deadline(deadline)
            .send()
            .map_err(|_| http::Error::IoError)?;

        let response = pending
            .try_wait(deadline)
            .map_err(|_| http::Error::DeadlineReached)??;

        if response.code != 200 {
            log::warn!("Unexpected status code: {}", response.code);
            return Err(http::Error::Unknown);
        }

        let body = response.body().collect::<Vec<u8>>();
        let body_str = str::from_utf8(&body).map_err(|_| {
            log::warn!("Response was not valid UTF8");
            http::Error::Unknown
        })?;

        log::info!("Got price response: {}", body_str);

        // Parse the JSON response
        let price_data = serde_json::from_str::<CryptoCompareResponse>(body_str).map_err(|e| {
            log::warn!("Failed to parse price from response: {:?}", e);
            http::Error::Unknown
        })?;

        // price has 3 decimals
        let price = (price_data.usd * 1000.0) as u32;
        log::info!("ADA price * 1000: {}", price);

        Ok(price)
    }
}
