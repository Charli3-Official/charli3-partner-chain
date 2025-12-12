use super::statistics::{quantile, Rational};
use sp_std::vec::Vec;

/// It will check for underflow/overflow and list size limitations (>0) and return None in that cases.
pub fn calculate_median(mut prices: Vec<u64>) -> Option<u64> {
    match prices.len() {
        0 => None,
        1 => Some(prices[0]),
        _ => {
            prices.sort_unstable();
            quantile(prices, Rational::new(1, 2))
                .and_then(|v| v.round().to_integer().try_into().ok())
        }
    }
}
