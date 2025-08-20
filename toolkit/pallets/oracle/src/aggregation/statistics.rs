use num_rational::Ratio;
use num_traits::ops::checked::{CheckedAdd, CheckedMul};
use sp_runtime::traits::CheckedSub;
use sp_std::vec::Vec;

pub type Rational = Ratio<u128>;

const SCALING_FACTOR: u128 = 1000;
const PERCENT: u128 = 100;
const IQR_THRESHOLD: usize = 4;

/// Returns weighted (by proximity) average of the two elements closest to the quantile index q * (n - 1)
/// It will also check for underflow/overflow and list size limitations (>1) and return None in that cases.
pub fn quantile(sorted_prices: Vec<u32>, q: Rational) -> Option<Rational> {
    let length: u128 = sorted_prices.len().try_into().ok()?;

    if length <= 1 {
        return sorted_prices
            .first()
            .map(|&x| Rational::from_integer(x.into()));
    }

    // Calculate quantile index: q * (length - 1)
    let n_minus_one = Rational::from_integer(length.checked_sub(1)?);
    let index = q.checked_mul(&n_minus_one)?;

    // Integral part of the quantile index
    let lower_idx: usize = index.floor().to_integer().try_into().ok()?;
    // Fractional part of the quantile index
    let weight = index.checked_sub(&index.floor())?;

    if lower_idx + 1 >= sorted_prices.len() {
        return Some(Rational::from_integer(sorted_prices[lower_idx].into()));
    }

    let lower_val = Rational::from_integer(sorted_prices[lower_idx].into());
    let upper_val = Rational::from_integer(sorted_prices[lower_idx + 1].into());

    // Linearly interpolate between lower_val and upper_val, using weight as the mixing factor.
    let weighted_lower = (Rational::from_integer(1) - weight).checked_mul(&lower_val)?;
    let weighted_upper = weight.checked_mul(&upper_val)?;

    weighted_lower.checked_add(&weighted_upper)
}

pub fn filter_outliers(
    prices: Vec<u32>,
    median: u32,
    outliers_range: u32,
    divergency: u32,
) -> Option<(Vec<u32>, Vec<u32>)> {
    if prices.len() <= 1 {
        return Some((prices, Vec::new()));
    }

    if prices.len() < IQR_THRESHOLD {
        return Some(
            prices
                .into_iter()
                .partition(|&price| is_within_divergency(price, median, divergency)),
        );
    }

    filter_outliers_iqr(prices, median, outliers_range, divergency)
}

fn filter_outliers_iqr(
    prices: Vec<u32>,
    median: u32,
    outliers_range: u32,
    divergency: u32,
) -> Option<(Vec<u32>, Vec<u32>)> {
    let mut sorted_prices = prices.clone();
    sorted_prices.sort_unstable();

    let q1 = quantile(sorted_prices.clone(), Rational::new(25, PERCENT))?;
    let q3 = quantile(sorted_prices, Rational::new(75, PERCENT))?;
    let iqr = q3 - q1;

    if iqr.round().to_integer() == 0 {
        log::warn!("IQR equals zero, falling back to divergency method");
        return Some(
            prices
                .into_iter()
                .partition(|&price| is_within_divergency(price, median, divergency)),
        );
    }

    let multiplier = Rational::new(outliers_range.into(), PERCENT);
    let fence = multiplier.checked_mul(&iqr)?;

    let lower_bound = (q1 - fence).round().to_integer().max(0) as u32;
    let upper_bound = (q3 + fence).round().to_integer() as u32;

    Some(
        prices
            .into_iter()
            .partition(|&price| price >= lower_bound && price <= upper_bound),
    )
}

fn is_within_divergency(price: u32, median: u32, divergency: u32) -> bool {
    if median == 0 {
        return price == 0;
    }

    let diff = if price > median {
        price - median
    } else {
        median - price
    };
    let ratio = (diff as u128 * SCALING_FACTOR) / median as u128;
    ratio <= divergency as u128
}
