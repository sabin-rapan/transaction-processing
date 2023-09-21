#![deny(missing_docs)]
#![deny(warnings)]

use serde::Deserialize;

/// Transaction ID.
pub type Id = u32;

/// Supported types of transactions.
#[derive(Copy, Clone, Deserialize, PartialEq, Debug)]
pub enum TransactionType {
    /// Deposit transaction.
    #[serde(alias = "deposit")]
    Deposit,
    #[serde(alias = "withdrawal")]
    /// Withdrawal transaction.
    Withdrawal,
    #[serde(alias = "dispute")]
    /// Dispute either a deposit or a withdrawal transaction.
    ///
    /// Clients dispute a deposit when their account was funded erroneously.
    /// Clients dispute a withdrawal when someone else took funds from their account without their
    /// consent (this is like a charge back but from exchange owner to client bank).
    Dispute,
    #[serde(alias = "resolve")]
    /// Resolve a dispute transaction.
    Resolve,
    /// Charge back a disputed deposit transaction.
    #[serde(alias = "chargeback")]
    ChargeBack,
}

/// Transaction data structure used as API payload.
#[derive(Copy, Clone, Deserialize, Debug)]
pub struct TransactionRecord {
    #[serde(alias = "type")]
    pub transaction_type: TransactionType,
    pub client: crate::model::account::Id,
    #[serde(alias = "tx")]
    pub id: Id,
    pub amount: Option<f64>,
}

impl std::fmt::Display for TransactionRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Record type {:?} client {} id {} amount {:?}",
            self.transaction_type, self.client, self.id, self.amount
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deser() {
        let data = r#"{"type":"deposit","client":1234,"tx":5678,"amount":1.2}"#;
        let transaction: TransactionRecord = serde_json::from_str(data).unwrap();
        assert_eq!(transaction.transaction_type, TransactionType::Deposit);
        assert_eq!(transaction.client, 1234);
        assert_eq!(transaction.id, 5678);
        assert_eq!(transaction.amount, Some(1.2));
    }
}
