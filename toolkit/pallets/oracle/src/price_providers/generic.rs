use super::PriceProvider;
use crate::aggregation::{calculate_median, filter_outliers};
use crate::config::{DataSource, JsonPathElement, NodeConfig};
use serde_json::Value;
use sp_runtime::offchain::http;
use sp_runtime::sp_std::{str, vec::Vec};

pub struct GenericApiProvider;

impl GenericApiProvider {
    fn load_config() -> Result<NodeConfig, &'static str> {
        match sp_io::offchain::local_storage_get(
            sp_core::offchain::StorageKind::PERSISTENT,
            b"node_config",
        ) {
            Some(config_bytes) => {
                let config_str =
                    str::from_utf8(&config_bytes).map_err(|_| "Invalid UTF-8 in config")?;

                NodeConfig::from_json_str(config_str)
            }
            None => {
                log::warn!("No node config found in storage, using default configuration");
                Ok(NodeConfig::default())
            }
        }
    }

    fn fetch_from_source(source: &DataSource) -> Result<f64, http::Error> {
        let name = source.name_str().unwrap_or("unknown");
        let url = source.url_str().map_err(|_| {
            log::error!("Invalid URL for source {}", name);
            http::Error::Unknown
        })?;

        log::info!("Fetching price from source: {}", name);

        let mut request = http::Request::get(url);

        for (header_name, header_value) in &source.headers {
            if let (Ok(name_str), Ok(value_str)) =
                (str::from_utf8(header_name), str::from_utf8(header_value))
            {
                request = request.add_header(name_str, value_str);
            }
        }

        let pending = request.send().map_err(|_| {
            log::error!("Failed to send request to {}", name);
            http::Error::IoError
        })?;

        let response = pending.wait().map_err(|_| {
            log::error!("Request failed for source {}", name);
            http::Error::Unknown
        })?;

        if response.code != 200 {
            log::warn!("HTTP error {} from source {}", response.code, name);
            return Err(http::Error::Unknown);
        }

        let body = response.body().collect::<Vec<u8>>();
        let body_str = str::from_utf8(&body).map_err(|_| {
            log::warn!("Invalid UTF-8 response from source {}", name);
            http::Error::Unknown
        })?;

        log::debug!("Response from {}: {}", name, body_str);

        Self::extract_price_from_json(body_str, &source.json_path).ok_or_else(|| {
            log::warn!("Failed to extract price from source {}", name);
            http::Error::Unknown
        })
    }

    fn extract_price_from_json(json_str: &str, path: &[JsonPathElement]) -> Option<f64> {
        let value: Value = serde_json::from_str(json_str).ok()?;
        let mut current = &value;

        for element in path {
            match element {
                JsonPathElement::String(_) => {
                    if let Some(Ok(key_str)) = element.key_str() {
                        current = current.get(key_str)?;
                    } else {
                        return None;
                    }
                }
                JsonPathElement::Number(_) => {
                    if let Some(index) = element.index() {
                        current = current.get(index as usize)?;
                    } else {
                        return None;
                    }
                }
            }
        }

        match current {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse::<f64>().ok(),
            Value::Array(arr) => {
                if let Some(first) = arr.first() {
                    match first {
                        Value::Number(n) => n.as_f64(),
                        Value::String(s) => s.parse::<f64>().ok(),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn aggregate_prices_with_iqr(prices: Vec<f64>) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }

        let mut sorted_prices = prices;
        sorted_prices.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let scaled_prices: Vec<u32> = sorted_prices.iter().map(|p| (*p * 1000.0) as u32).collect();

        let outliers_range = 150;
        let divergency = 50;

        if let Some(median_u32) = calculate_median(scaled_prices.clone()) {
            if let Some((non_outliers, outliers)) =
                filter_outliers(scaled_prices, median_u32, outliers_range, divergency)
            {
                log::info!(
                    "Source aggregation: {} valid prices, {} outliers filtered",
                    non_outliers.len(),
                    outliers.len()
                );

                if let Some(final_median) = calculate_median(non_outliers) {
                    return Some(final_median as f64 / 1000.0);
                }
            }
        }

        None
    }

    fn validate_price(price: f64) -> bool {
        price > 0.0
    }
}

impl PriceProvider for GenericApiProvider {
    fn fetch_price() -> Result<u32, http::Error> {
        let config = Self::load_config().map_err(|e| {
            log::error!("Failed to load config: {}", e);
            http::Error::Unknown
        })?;

        if config.sources.is_empty() {
            log::error!("No sources configured in oracle config");
            return Err(http::Error::Unknown);
        }

        let mut successful_prices = Vec::new();
        let mut errors = 0;

        for source in &config.sources {
            match Self::fetch_from_source(source) {
                Ok(price) => {
                    if Self::validate_price(price) {
                        let name = source.name_str().unwrap_or("unknown");
                        log::info!("Successfully fetched price {} from {}", price, name);
                        successful_prices.push(price);
                    } else {
                        let name = source.name_str().unwrap_or("unknown");
                        log::warn!("Invalid price {} from source {}", price, name);
                        errors += 1;
                    }
                }
                Err(e) => {
                    let name = source.name_str().unwrap_or("unknown");
                    log::warn!("Failed to fetch from {}: {:?}", name, e);
                    errors += 1;
                }
            }
        }

        if successful_prices.is_empty() {
            log::error!(
                "All {} sources failed or returned invalid prices",
                config.sources.len()
            );
            return Err(http::Error::Unknown);
        }

        log::info!(
            "Successfully fetched from {}/{} sources (errors: {})",
            successful_prices.len(),
            config.sources.len(),
            errors
        );

        let aggregated_price =
            Self::aggregate_prices_with_iqr(successful_prices).ok_or_else(|| {
                log::error!("Failed to aggregate prices using IQR method");
                http::Error::Unknown
            })?;

        let price_scaled = (aggregated_price * 1000.0) as u32;
        log::info!(
            "Aggregated price * 1000 (with IQR filtering): {}",
            price_scaled
        );

        Ok(price_scaled)
    }
}