use codec::alloc::string::{String, ToString};
use hex;
use num_traits::float::FloatCore;
use serde::{Deserialize, Serialize};
use sp_runtime::format;
use sp_runtime::offchain::{http, Duration};
use sp_runtime::sp_std::borrow::ToOwned;
use sp_runtime::sp_std::str;
use sp_runtime::Vec;
use sp_std::collections::btree_map::BTreeMap;

pub trait PriceProvider {
    fn fetch_prices(tickers: Vec<String>) -> Result<Vec<u32>, http::Error>;
}

pub struct CryptoCompareProvider;

const CRYPTOCOMPARE_API_KEY_DEFAULT: &str = "";

type CryptoCompareResponse = BTreeMap<String, BTreeMap<String, f64>>;

/// Trade Pair measures price of base (from) currency in terms of quote (to) currency.
/// E.g. ADA-USD (BASE-QUOTE) price tells a price of 1 ADA in USD.
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct CryptoCompareTradePair {
    /// Base aka from currency, e.g. ADA
    base_currency: String,
    /// Quote aka to currency, e.g. USD
    quote_currency: String,
}

impl CryptoCompareTradePair {
    /// Create a Trade Pair from BASE and QUOTE currencies separated by delimiter:
    /// '_' ' ' '/' '-' '.' are accepted as delimiters.
    /// For example ADA-USD, where ADA is a base currency and USD is a quote currency.
    pub fn from_ticker(ticker: &str) -> Self {
        match ticker
            .split(|char| char == ' ' || char == '/' || char == '-' || char == '.' || char == '_')
            .collect::<Vec<&str>>()
            .as_slice()
        {
            [base, quote] => CryptoCompareTradePair {
                base_currency: base.to_uppercase().to_string(),
                quote_currency: quote.to_uppercase().to_string(),
            },
            _ => panic!["TradePair.from_ticker parse error."],
        }
    }
}

impl PriceProvider for CryptoCompareProvider {
    fn fetch_prices(tickers: Vec<String>) -> Result<Vec<u32>, http::Error> {
        // Get API key from offchain storage if available
        let api_key = match sp_io::offchain::local_storage_get(
            sp_core::offchain::StorageKind::PERSISTENT,
            b"cryptocompare_api_key",
        ) {
            Some(stored_key) => {
                // key is stored as bytes, convert to hex
                let key_in_hex = hex::encode(stored_key);
                key_in_hex
            }
            _none => {
                log::warn!("No API key found in storage, using default: no key");
                CRYPTOCOMPARE_API_KEY_DEFAULT.to_owned()
            }
        };

        let trade_pairs: Vec<CryptoCompareTradePair> = tickers
            .iter()
            .map(|t| CryptoCompareTradePair::from_ticker(t))
            .collect();
        let from_syms: String = trade_pairs
            .iter()
            .map(|p| p.base_currency.as_str())
            .collect::<Vec<&str>>()
            .join(",");
        let to_syms: String = trade_pairs
            .iter()
            .map(|p| p.quote_currency.as_str())
            .collect::<Vec<&str>>()
            .join(",");

        let url = format!(
            "https://min-api.cryptocompare.com/data/pricemulti?fsyms={}&tsyms={}&api_key={}",
            from_syms, to_syms, api_key
        );

        let request = http::Request::get(url.as_str());

        // 2 seconds timeout for not hanging the node
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));

        let pending = request
            .deadline(deadline)
            .send()
            .map_err(|_| http::Error::IoError)?;

        let response: http::Response = pending
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
        let price_data: CryptoCompareResponse =
            serde_json::from_str::<CryptoCompareResponse>(body_str).map_err(|e| {
                log::warn!("Failed to parse price from response: {:?}", e);
                http::Error::Unknown
            })?;

        trade_pairs
            .iter()
            .map(|tp| {
                let quote_mapping = price_data.get(&tp.base_currency).ok_or_else(|| {
                    log::warn!("Could not find base currency {}", &tp.base_currency);
                    http::Error::Unknown
                });

                quote_mapping.and_then(|quote| {
                    quote
                        .get(&tp.quote_currency)
                        .ok_or_else(|| {
                            log::warn!("Could not find quote currency {}", &tp.quote_currency);
                            http::Error::Unknown
                        })
                        .map(|p| {
                            let price = FloatCore::round(p * 1000.0) as u32;
                            log::info!(
                                "{}.{} price * 1000: {}",
                                &tp.base_currency,
                                &tp.quote_currency,
                                price
                            );
                            price
                        })
                })
            })
            .collect()
    }
}
