#![deny(missing_docs)]
#![deny(warnings)]

use rust_decimal::prelude::*;
use serde::{Serialize, Serializer};

/// Used to express currency amounts
///
/// It's implemented as a thin wrapper over rust_decimal's Decimal due to the crate's support for
/// significant decimal and fractionary digits and no round-off errors in addition to checked
/// mathematical operations which handle overflow and underflow.
/// Behind the scenes, rust_decimal uses multiple unsigned integers to represent fractional
/// numbers, which is definitely superior to the non-sense that is f64.
///
/// It is noteworthy to mention `f64::MAX` is greater than `Amount::MAX`, thus not all f64 values
/// can be represented with an `Amount`.
///
/// We deliberately make a decision to hide the internal representation, as it might change in the
/// future. Most standard mathematical operations are not implemented as they are not needed at
/// this point in time, thus they are left as an exercise for the reader.
///
/// Serialization is done by rounding the amount to 4 decimal points, thus serialized data is
/// suitable only for human inspection, not for sending it over a write protocol.
///
/// Amount::ZERO, Amount::MIN, Amount::MAX are declared to make it clear what are the bounds of the
/// amount, even though they are not used except in tests.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount(Decimal);

impl Amount {
    /// The zero amount.
    #[allow(dead_code)]
    pub const ZERO: Amount = Amount(Decimal::ZERO);
    /// The minimum value of an amount.
    #[allow(dead_code)]
    pub const MIN: Amount = Amount(Decimal::MIN);
    /// The maximum value of an amount.
    #[allow(dead_code)]
    pub const MAX: Amount = Amount(Decimal::MAX);

    /// Checked addition.
    /// Returns `None` if overflow occurred.
    pub fn checked_add(&self, rhs: Amount) -> Option<Amount> {
        self.0.checked_add(rhs.0).map(Amount)
    }

    /// Checked subtraction.
    /// Returns `None` if overflow occurred.
    pub fn checked_sub(&self, rhs: Amount) -> Option<Amount> {
        self.0.checked_sub(rhs.0).map(Amount)
    }

    /// Converts a `f64` to return an optional value of this type. If the value cannot be
    /// represented by this type, then `None` is returned.
    pub fn from_f64(amount: f64) -> Option<Self> {
        Decimal::from_f64(amount).map(Amount)
    }
}

impl Serialize for Amount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("Amount", &self.0.round_dp(4))
    }
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_default() {
        assert_eq!(Amount::default(), Amount::ZERO);
    }

    #[test]
    fn test_constants() {
        assert_eq!(Amount::MIN.0, Decimal::MIN);
        assert_eq!(Amount::MAX.0, Decimal::MAX);
        assert_eq!(Amount::ZERO.0, Decimal::ZERO);
    }

    #[test]
    fn test_serialize() {
        let expected = r#""1.2346""#;
        assert_eq!(
            serde_json::to_string(&Amount::from_f64(1.23456789)).unwrap(),
            expected
        );
    }

    #[test]
    fn test_f64_conversion() {
        assert!(Amount::from_f64(f64::MAX).is_none());
        assert!(Amount::from_f64(f64::MIN).is_none());
        assert_eq!(Amount::from_f64(0.0).unwrap(), Amount::ZERO);
    }
}
