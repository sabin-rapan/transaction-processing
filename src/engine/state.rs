#![deny(missing_docs)]
#![deny(warnings)]

use crate::model::account::{Account, Id as AccountId};
use crate::model::amount::Amount;
use crate::model::transaction::{Id as TransactionId, TransactionRecord, TransactionType};
use std::collections::HashMap;
use std::convert::TryFrom;

/// Error conditions that may arise when using this module.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// Invalid account operation.
    #[error("Failed to execute transaction due to account error")]
    Account(#[from] crate::model::account::Error),
    /// Invalid deposit transaction.
    #[error("Invalid deposit")]
    Deposit,
    /// Invalid withdrawal transaction.
    #[error("Invalid withdrawal")]
    Withdrawal,
    /// Invalid dispute transaction.
    #[error("Invalid dispute")]
    Dispute,
    /// Invalid resolve transaction.
    #[error("Invalid resolve")]
    Resolve,
    /// Invalid charge back transaction.
    #[error("Invalid charge back")]
    ChargeBack,
    /// Deposit/Withdrawal with same id.
    #[error("Duplicate transaction")]
    DuplicateTransactionId,
    /// Transaction for another account id.
    #[error("Invalid account id")]
    InvalidAccountId,
}

/// Result of account operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Internal data representation of a transaction metadata.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TransactionMetadata(pub TransactionId, pub AccountId);

/// Internal data representation of a transaction.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Transaction {
    /// Deposit transaction.
    Deposit(TransactionMetadata, Amount, bool),
    /// Withdrawal transaction.
    Withdrawal(TransactionMetadata, Amount),
    /// Dispute transaction.
    Dispute(TransactionMetadata),
    /// Resolve transaction.
    Resolve(TransactionMetadata),
    /// Charge back transaction.
    ChargeBack(TransactionMetadata),
}

impl Transaction {
    pub fn apply(&self, state: &mut State) -> Result<()> {
        match self {
            Self::Deposit(_, _, _) => self.deposit(state),
            Self::Withdrawal(_, _) => self.withdrawal(state),
            Self::Dispute(_) => self.dispute(state),
            Self::Resolve(_) => self.resolve(state),
            Self::ChargeBack(_) => self.charge_back(state),
        }
    }

    fn deposit(&self, state: &mut State) -> Result<()> {
        match self {
            Self::Deposit(md, amount, _) => {
                if state.account.id() != md.1 {
                    return Err(Error::InvalidAccountId);
                }
                if state.transaction_history.contains_key(&md.0) {
                    return Err(Error::DuplicateTransactionId);
                }
                state.account.deposit(*amount).map_err(Error::Account)?;
                state.transaction_history.insert(md.0, *self);

                Ok(())
            }
            _ => Err(Error::Deposit),
        }
    }

    fn withdrawal(&self, state: &mut State) -> Result<()> {
        match self {
            Self::Withdrawal(md, amount) => {
                if state.account.id() != md.1 {
                    return Err(Error::InvalidAccountId);
                }
                if state.transaction_history.contains_key(&md.0) {
                    return Err(Error::DuplicateTransactionId);
                };
                state.account.withdrawal(*amount).map_err(Error::Account)?;
                state.transaction_history.insert(md.0, *self);

                Ok(())
            }
            _ => Err(Error::Withdrawal),
        }
    }

    fn dispute(&self, state: &mut State) -> Result<()> {
        match self {
            Self::Dispute(md) => {
                if state.account.id() != md.1 {
                    return Err(Error::InvalidAccountId);
                }

                let disputed_transaction =
                    state.transaction_history.get(&md.0).ok_or(Error::Dispute)?;

                match disputed_transaction {
                    Self::Deposit(md, amount, is_disputed) => {
                        if *is_disputed {
                            return Err(Error::Dispute);
                        }
                        state.account.dispute(*amount).map_err(Error::Account)?;
                        state
                            .transaction_history
                            .insert(md.0, Self::Deposit(*md, *amount, true));
                    }
                    _ => {
                        return Err(Error::Dispute);
                    }
                }

                Ok(())
            }
            _ => Err(Error::Dispute),
        }
    }

    fn resolve(&self, state: &mut State) -> Result<()> {
        match self {
            Self::Resolve(md) => {
                if state.account.id() != md.1 {
                    return Err(Error::InvalidAccountId);
                }

                let disputed_transaction =
                    state.transaction_history.get(&md.0).ok_or(Error::Resolve)?;

                match disputed_transaction {
                    Self::Deposit(md, amount, is_disputed) => {
                        if !*is_disputed {
                            return Err(Error::Resolve);
                        }
                        state.account.resolve(*amount).map_err(Error::Account)?;
                        state
                            .transaction_history
                            .insert(md.0, Self::Deposit(*md, *amount, false));

                        Ok(())
                    }
                    _ => Err(Error::Resolve),
                }
            }
            _ => Err(Error::Resolve),
        }
    }

    fn charge_back(&self, state: &mut State) -> Result<()> {
        match self {
            Self::ChargeBack(md) => {
                if state.account.id() != md.1 {
                    return Err(Error::InvalidAccountId);
                }

                let disputed_transaction = state
                    .transaction_history
                    .get(&md.0)
                    .ok_or(Error::ChargeBack)?;

                match disputed_transaction {
                    Self::Deposit(md, amount, is_disputed) => {
                        if !*is_disputed {
                            return Err(Error::ChargeBack);
                        }
                        state.account.charge_back(*amount).map_err(Error::Account)?;
                        state
                            .transaction_history
                            .insert(md.0, Self::Deposit(*md, *amount, false));

                        Ok(())
                    }
                    _ => Err(Error::ChargeBack),
                }
            }
            _ => Err(Error::ChargeBack),
        }
    }
}

impl TryFrom<TransactionRecord> for Transaction {
    type Error = crate::engine::state::Error;

    fn try_from(tx: TransactionRecord) -> Result<Self> {
        match tx.transaction_type {
            TransactionType::Deposit => Ok(Self::Deposit(
                TransactionMetadata(tx.id, tx.client),
                Amount::from_f64(tx.amount.ok_or(Error::Deposit)?).ok_or(Error::Deposit)?,
                false,
            )),
            TransactionType::Withdrawal => Ok(Self::Withdrawal(
                TransactionMetadata(tx.id, tx.client),
                Amount::from_f64(tx.amount.ok_or(Error::Withdrawal)?).ok_or(Error::Withdrawal)?,
            )),
            TransactionType::Dispute => Ok(Self::Dispute(TransactionMetadata(tx.id, tx.client))),
            TransactionType::Resolve => Ok(Self::Resolve(TransactionMetadata(tx.id, tx.client))),
            TransactionType::ChargeBack => {
                Ok(Self::ChargeBack(TransactionMetadata(tx.id, tx.client)))
            }
        }
    }
}

impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Transaction::Deposit(md, amount, is_disputed) => write!(
                f,
                "Deposit id {} client {} amount {} is_disputed {}",
                md.0, md.1, amount, is_disputed
            ),
            Transaction::Withdrawal(md, amount) => {
                write!(f, "Withdraw id {} client {} amount {}", md.0, md.1, amount)
            }
            Transaction::Dispute(md) => write!(f, "Dispute id {}", md.0),
            Transaction::Resolve(md) => write!(f, "Resolve id {}", md.0),
            Transaction::ChargeBack(md) => write!(f, "Charge back id {}", md.0),
        }
    }
}

/// State of all a client account.
#[derive(Default)]
pub struct State {
    /// Account
    pub account: Account,
    /// History of deposits and withdrawals.
    pub transaction_history: HashMap<TransactionId, Transaction>,
}

impl State {
    pub fn new(id: AccountId) -> Self {
        Self {
            account: Account::new(id),
            transaction_history: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::account::Error as AccountError;

    #[test]
    fn test_state_default() {
        assert_eq!(State::default().account, Account::default());
        assert!(State::default().transaction_history.is_empty());
    }

    #[test]
    fn test_transaction_tryfrom() {
        assert_eq!(
            Transaction::try_from(TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client: 1,
                id: 2,
                amount: Some(1.0)
            })
            .unwrap(),
            Transaction::Deposit(
                TransactionMetadata(2, 1),
                Amount::from_f64(1.0).unwrap(),
                false
            )
        );
        assert_eq!(
            Transaction::try_from(TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client: 1,
                id: 2,
                amount: Some(1.0)
            })
            .unwrap(),
            Transaction::Withdrawal(TransactionMetadata(2, 1), Amount::from_f64(1.0).unwrap())
        );
        assert_eq!(
            Transaction::try_from(TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client: 1,
                id: 2,
                amount: None
            })
            .unwrap(),
            Transaction::Dispute(TransactionMetadata(2, 1))
        );
        assert_eq!(
            Transaction::try_from(TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client: 1,
                id: 2,
                amount: None
            })
            .unwrap(),
            Transaction::Resolve(TransactionMetadata(2, 1))
        );
        assert_eq!(
            Transaction::try_from(TransactionRecord {
                transaction_type: TransactionType::ChargeBack,
                client: 1,
                id: 2,
                amount: None
            })
            .unwrap(),
            Transaction::ChargeBack(TransactionMetadata(2, 1))
        );
        assert!(Transaction::try_from(TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client: 1,
            id: 2,
            amount: None
        })
        .is_err());
        assert!(Transaction::try_from(TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client: 1,
            id: 2,
            amount: None
        })
        .is_err());
    }

    #[test]
    fn test_transaction_apply() {
        let mut state = State::new(1);

        // Same transaction id deposit test-case
        let deposit = Transaction::Deposit(TransactionMetadata(1, 1), Amount::MAX, false);
        deposit.apply(&mut state).unwrap();
        assert_eq!(
            deposit.apply(&mut state).err().unwrap(),
            Error::DuplicateTransactionId
        );

        // Deposit overflow test-case
        let deposit = Transaction::Deposit(TransactionMetadata(2, 1), Amount::MAX, false);
        assert_eq!(
            deposit.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::Overflow)
        );

        // Same transaction id withdrawal test-case
        let withdrawal = Transaction::Withdrawal(TransactionMetadata(3, 1), Amount::MAX);
        withdrawal.apply(&mut state).unwrap();
        assert_eq!(
            withdrawal.apply(&mut state).err().unwrap(),
            Error::DuplicateTransactionId
        );

        // Withdrawal insufficient funds test-case
        let withdrawal = Transaction::Withdrawal(TransactionMetadata(4, 1), Amount::MAX);
        assert_eq!(
            withdrawal.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::InsufficientFunds)
        );

        // Dispute deposit twice test-case
        let deposit = Transaction::Deposit(
            TransactionMetadata(5, 1),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(5, 1));
        dispute.apply(&mut state).unwrap();
        assert_eq!(dispute.apply(&mut state).err().unwrap(), Error::Dispute);

        // Resolve dispute twice test-case
        let resolve = Transaction::Resolve(TransactionMetadata(5, 1));
        resolve.apply(&mut state).unwrap();
        assert_eq!(resolve.apply(&mut state).err().unwrap(), Error::Resolve);

        // Charge back twice test-case
        let deposit = Transaction::Deposit(
            TransactionMetadata(6, 1),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(6, 1));
        dispute.apply(&mut state).unwrap();
        let charge_back = Transaction::ChargeBack(TransactionMetadata(6, 1));
        charge_back.apply(&mut state).unwrap();
        assert_eq!(
            charge_back.apply(&mut state).err().unwrap(),
            Error::ChargeBack
        );

        // Dispute/Resolve/ChargeBack on invalid transaction id
        let mut state = State::new(2);
        let deposit = Transaction::Deposit(
            TransactionMetadata(7, 2),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(1234, 2));
        assert_eq!(dispute.apply(&mut state).err().unwrap(), Error::Dispute);
        let resolve = Transaction::Resolve(TransactionMetadata(1234, 2));
        assert_eq!(resolve.apply(&mut state).err().unwrap(), Error::Resolve);
        let charge_back = Transaction::ChargeBack(TransactionMetadata(1234, 2));
        assert_eq!(
            charge_back.apply(&mut state).err().unwrap(),
            Error::ChargeBack
        );

        // Resolve/ChargeBack on undisputed transaction id
        let resolve = Transaction::Resolve(TransactionMetadata(7, 2));
        assert_eq!(resolve.apply(&mut state).err().unwrap(), Error::Resolve);
        let charge_back = Transaction::ChargeBack(TransactionMetadata(7, 2));
        assert_eq!(
            charge_back.apply(&mut state).err().unwrap(),
            Error::ChargeBack
        );

        // Locked account test-case
        let deposit = Transaction::Deposit(
            TransactionMetadata(8, 2),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(8, 2));
        dispute.apply(&mut state).unwrap();

        let deposit = Transaction::Deposit(
            TransactionMetadata(9, 2),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();

        let deposit = Transaction::Deposit(
            TransactionMetadata(10, 2),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(10, 2));
        dispute.apply(&mut state).unwrap();

        let deposit = Transaction::Deposit(
            TransactionMetadata(11, 2),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(11, 2));
        dispute.apply(&mut state).unwrap();

        let charge_back = Transaction::ChargeBack(TransactionMetadata(8, 2));
        charge_back.apply(&mut state).unwrap();

        let deposit = Transaction::Deposit(
            TransactionMetadata(13, 2),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        assert_eq!(
            deposit.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::Locked)
        );
        let dispute = Transaction::Dispute(TransactionMetadata(9, 2));
        assert_eq!(
            dispute.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::Locked)
        );
        let resolve = Transaction::Resolve(TransactionMetadata(10, 2));
        assert_eq!(
            resolve.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::Locked)
        );
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(12, 2), Amount::from_f64(1.0).unwrap());
        assert_eq!(
            withdrawal.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::Locked)
        );
        let charge_back = Transaction::ChargeBack(TransactionMetadata(11, 2));
        assert_eq!(
            charge_back.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::Locked)
        );

        // Dispute/Resolve/ChargeBack on withdrawal
        let mut state = State::new(3);
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 3),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        deposit.apply(&mut state).unwrap();
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(2, 3), Amount::from_f64(1.0).unwrap());
        withdrawal.apply(&mut state).unwrap();
        let dispute = Transaction::Dispute(TransactionMetadata(2, 3));
        assert_eq!(dispute.apply(&mut state).err().unwrap(), Error::Dispute);
        let resolve = Transaction::Resolve(TransactionMetadata(2, 3));
        assert_eq!(resolve.apply(&mut state).err().unwrap(), Error::Resolve);
        let charge_back = Transaction::ChargeBack(TransactionMetadata(2, 3));
        assert_eq!(
            charge_back.apply(&mut state).err().unwrap(),
            Error::ChargeBack
        );

        // Deposit/Withdrawal invalid amount
        let mut state = State::new(4);
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 4),
            Amount::from_f64(-1.0).unwrap(),
            false,
        );
        assert_eq!(
            deposit.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::InvalidInput)
        );
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(2, 4), Amount::from_f64(-1.0).unwrap());
        assert_eq!(
            withdrawal.apply(&mut state).err().unwrap(),
            Error::Account(AccountError::InvalidInput)
        );

        // Transaction for another account id
        let mut state = State::new(5);
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 1234),
            Amount::from_f64(-1.0).unwrap(),
            false,
        );
        assert_eq!(
            deposit.apply(&mut state).err().unwrap(),
            Error::InvalidAccountId
        );
        let withdrawal = Transaction::Withdrawal(
            TransactionMetadata(2, 1234),
            Amount::from_f64(-1.0).unwrap(),
        );
        assert_eq!(
            withdrawal.apply(&mut state).err().unwrap(),
            Error::InvalidAccountId
        );
        let dispute = Transaction::Dispute(TransactionMetadata(1, 1234));
        assert_eq!(
            dispute.apply(&mut state).err().unwrap(),
            Error::InvalidAccountId
        );
        let resolve = Transaction::Resolve(TransactionMetadata(1, 1234));
        assert_eq!(
            resolve.apply(&mut state).err().unwrap(),
            Error::InvalidAccountId
        );
        let charge_back = Transaction::ChargeBack(TransactionMetadata(1, 1234));
        assert_eq!(
            charge_back.apply(&mut state).err().unwrap(),
            Error::InvalidAccountId
        );

        // Test private functions
        let mut state = State::new(6);
        // Deposit fn called on non-deposit transactions
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(1, 6), Amount::from_f64(1.0).unwrap());
        assert_eq!(
            withdrawal.deposit(&mut state).err().unwrap(),
            Error::Deposit
        );
        let dispute = Transaction::Dispute(TransactionMetadata(1, 1234));
        assert_eq!(dispute.deposit(&mut state).err().unwrap(), Error::Deposit);
        let resolve = Transaction::Resolve(TransactionMetadata(1, 1234));
        assert_eq!(resolve.deposit(&mut state).err().unwrap(), Error::Deposit);
        let charge_back = Transaction::ChargeBack(TransactionMetadata(1, 1234));
        assert_eq!(
            charge_back.deposit(&mut state).err().unwrap(),
            Error::Deposit
        );
        // Withdrawal fn called on non-withdrawal transactions
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 6),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        assert_eq!(
            deposit.withdrawal(&mut state).err().unwrap(),
            Error::Withdrawal
        );
        let dispute = Transaction::Dispute(TransactionMetadata(1, 1234));
        assert_eq!(
            dispute.withdrawal(&mut state).err().unwrap(),
            Error::Withdrawal
        );
        let resolve = Transaction::Resolve(TransactionMetadata(1, 1234));
        assert_eq!(
            resolve.withdrawal(&mut state).err().unwrap(),
            Error::Withdrawal
        );
        let charge_back = Transaction::ChargeBack(TransactionMetadata(1, 1234));
        assert_eq!(
            charge_back.withdrawal(&mut state).err().unwrap(),
            Error::Withdrawal
        );
        // Dispute fn called on non-dispute transactions
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 6),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        assert_eq!(deposit.dispute(&mut state).err().unwrap(), Error::Dispute);
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(1, 1234), Amount::from_f64(1.0).unwrap());
        assert_eq!(
            withdrawal.dispute(&mut state).err().unwrap(),
            Error::Dispute
        );
        let resolve = Transaction::Resolve(TransactionMetadata(1, 1234));
        assert_eq!(resolve.dispute(&mut state).err().unwrap(), Error::Dispute);
        let charge_back = Transaction::ChargeBack(TransactionMetadata(1, 1234));
        assert_eq!(
            charge_back.dispute(&mut state).err().unwrap(),
            Error::Dispute
        );
        // Resolve fn called on non-resolve transactions
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 6),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        assert_eq!(deposit.resolve(&mut state).err().unwrap(), Error::Resolve);
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(1, 1234), Amount::from_f64(1.0).unwrap());
        assert_eq!(
            withdrawal.resolve(&mut state).err().unwrap(),
            Error::Resolve
        );
        let dispute = Transaction::Dispute(TransactionMetadata(1, 1234));
        assert_eq!(dispute.resolve(&mut state).err().unwrap(), Error::Resolve);
        let charge_back = Transaction::ChargeBack(TransactionMetadata(1, 1234));
        assert_eq!(
            charge_back.resolve(&mut state).err().unwrap(),
            Error::Resolve
        );
        // ChargeBaack fn called on non-chargeback transactions
        let deposit = Transaction::Deposit(
            TransactionMetadata(1, 6),
            Amount::from_f64(1.0).unwrap(),
            false,
        );
        assert_eq!(
            deposit.charge_back(&mut state).err().unwrap(),
            Error::ChargeBack
        );
        let withdrawal =
            Transaction::Withdrawal(TransactionMetadata(1, 1234), Amount::from_f64(1.0).unwrap());
        assert_eq!(
            withdrawal.charge_back(&mut state).err().unwrap(),
            Error::ChargeBack
        );
        let dispute = Transaction::Dispute(TransactionMetadata(1, 1234));
        assert_eq!(
            dispute.charge_back(&mut state).err().unwrap(),
            Error::ChargeBack
        );
        let resolve = Transaction::Resolve(TransactionMetadata(1, 1234));
        assert_eq!(
            resolve.charge_back(&mut state).err().unwrap(),
            Error::ChargeBack
        );
    }
}
