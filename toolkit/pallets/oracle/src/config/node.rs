use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;
use codec::{Encode, Decode};
use scale_info::TypeInfo;
use scale_info::prelude::string::String;

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode, TypeInfo)]
pub struct NodeConfig {
    pub sources: Vec<DataSource>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode, TypeInfo)]
pub struct DataSource {
    pub name: Vec<u8>,
    pub url: Vec<u8>,
    pub json_path: Vec<JsonPathElement>,
    #[serde(default)]
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Encode, Decode, TypeInfo)]
#[serde(untagged)]
pub enum JsonPathElement {
    String(Vec<u8>),
    Number(u32),
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            sources: sp_std::vec![
                DataSource {
                    name: b"bitget".to_vec(),
                    url: b"https://api.bitget.com/api/spot/v1/market/ticker?symbol=ADAUSDC_SPBL".to_vec(),
                    json_path: sp_std::vec![
                        JsonPathElement::String(b"data".to_vec()),
                        JsonPathElement::String(b"close".to_vec())
                    ],
                    headers: sp_std::vec![],
                },
            ],
        }
    }
}

impl NodeConfig {
    pub fn from_json_str(json_str: &str) -> Result<Self, &'static str> {
        #[derive(Deserialize)]
        struct TempNodeConfig {
            sources: Vec<TempDataSource>,
        }

        #[derive(Deserialize)]
        struct TempDataSource {
            name: String,
            url: String,
            json_path: Vec<TempJsonPathElement>,
            #[serde(default)]
            headers: Vec<(String, String)>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TempJsonPathElement {
            String(String),
            Number(u32),
        }

        let temp_config: TempNodeConfig = serde_json::from_str(json_str)
            .map_err(|_| "Failed to parse JSON")?;

        let sources = temp_config.sources.into_iter().map(|temp_source| {
            DataSource {
                name: temp_source.name.into_bytes(),
                url: temp_source.url.into_bytes(),
                json_path: temp_source.json_path.into_iter().map(|temp_element| {
                    match temp_element {
                        TempJsonPathElement::String(s) => JsonPathElement::String(s.into_bytes()),
                        TempJsonPathElement::Number(n) => JsonPathElement::Number(n),
                    }
                }).collect(),
                headers: temp_source.headers.into_iter().map(|(k, v)| (k.into_bytes(), v.into_bytes())).collect(),
            }
        }).collect();

        Ok(NodeConfig { sources })
    }
}

impl DataSource {
    pub fn name_str(&self) -> Result<&str, &'static str> {
        core::str::from_utf8(&self.name).map_err(|_| "Invalid UTF-8 in name")
    }

    pub fn url_str(&self) -> Result<&str, &'static str> {
        core::str::from_utf8(&self.url).map_err(|_| "Invalid UTF-8 in URL")
    }
}

impl JsonPathElement {
    pub fn key_str(&self) -> Option<Result<&str, &'static str>> {
        match self {
            JsonPathElement::String(key) => Some(core::str::from_utf8(key).map_err(|_| "Invalid UTF-8 in key")),
            JsonPathElement::Number(_) => None,
        }
    }

    pub fn index(&self) -> Option<u32> {
        match self {
            JsonPathElement::Number(index) => Some(*index),
            JsonPathElement::String(_) => None,
        }
    }
}