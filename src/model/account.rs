#![deny(missing_docs)]
#![deny(warnings)]

use crate::model::amount::Amount;
use serde::Serialize;

/// Error conditions that may arise when creating a new `Account` objects.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("Account balance overflow")]
    Overflow,
    #[error("Account is locked")]
    Locked,
    #[error("Account has insufficient funds")]
    InsufficientFunds,
    #[error("Account operation has invalid input")]
    InvalidInput,
}

/// Result of account operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Client ID.
pub type Id = u16;

/// Used to express client account balances.
#[derive(Copy, Clone, Default, Debug, Serialize, PartialEq)]
pub struct Account {
    #[serde(rename = "client")]
    id: Id,
    available: Amount,
    held: Amount,
    total: Amount,
    locked: bool,
}

impl Account {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            available: Amount::default(),
            held: Amount::default(),
            total: Amount::default(),
            locked: false,
        }
    }

    #[allow(dead_code)]
    pub fn available(&self) -> Amount {
        self.available
    }

    #[allow(dead_code)]
    pub fn held(&self) -> Amount {
        self.held
    }

    #[allow(dead_code)]
    pub fn total(&self) -> Amount {
        self.total
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn id(&self) -> Id {
        self.id
    }

    #[allow(dead_code)]
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn deposit(&mut self, amount: Amount) -> Result<()> {
        if amount <= Amount::ZERO {
            return Err(Error::InvalidInput);
        }

        if self.locked() {
            return Err(Error::Locked);
        }

        self.available = self.available.checked_add(amount).ok_or(Error::Overflow)?;
        self.total = self.total.checked_add(amount).ok_or(Error::Overflow)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn dispute(&mut self, amount: Amount) -> Result<()> {
        if amount <= Amount::ZERO {
            return Err(Error::InvalidInput);
        }

        if self.locked() {
            return Err(Error::Locked);
        }

        let avail_diff = self.available.checked_sub(amount).ok_or(Error::Overflow)?;

        if avail_diff < Amount::ZERO {
            return Err(Error::InsufficientFunds);
        }

        self.held = self.held.checked_add(amount).ok_or(Error::Overflow)?;
        self.available = avail_diff;

        Ok(())
    }

    pub fn withdrawal(&mut self, amount: Amount) -> Result<()> {
        if amount <= Amount::ZERO {
            return Err(Error::InvalidInput);
        }

        if self.locked() {
            return Err(Error::Locked);
        }

        let avail_diff = self.available.checked_sub(amount).ok_or(Error::Overflow)?;

        if avail_diff < Amount::ZERO {
            return Err(Error::InsufficientFunds);
        }

        let total_diff = self.total.checked_sub(amount).ok_or(Error::Overflow)?;

        if total_diff < Amount::ZERO {
            return Err(Error::InsufficientFunds);
        }

        self.available = avail_diff;
        self.total = total_diff;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn resolve(&mut self, amount: Amount) -> Result<()> {
        if amount <= Amount::ZERO {
            return Err(Error::InvalidInput);
        }

        if self.locked() {
            return Err(Error::Locked);
        }

        let held_diff = self.held.checked_sub(amount).ok_or(Error::Overflow)?;

        if held_diff < Amount::ZERO {
            return Err(Error::InsufficientFunds);
        }

        self.available = self.available.checked_add(amount).ok_or(Error::Overflow)?;
        self.held = held_diff;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn charge_back(&mut self, amount: Amount) -> Result<()> {
        if amount <= Amount::ZERO {
            return Err(Error::InvalidInput);
        }

        if self.locked() {
            return Err(Error::Locked);
        }

        let held_diff = self.held.checked_sub(amount).ok_or(Error::Overflow)?;

        if held_diff < Amount::ZERO {
            return Err(Error::InsufficientFunds);
        }

        let total_diff = self.total.checked_sub(amount).ok_or(Error::Overflow)?;

        if total_diff < Amount::ZERO {
            return Err(Error::InsufficientFunds);
        }

        self.held = held_diff;
        self.total = total_diff;
        self.set_locked(true);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_default() {
        let account = Account::default();

        assert_eq!(account.id(), 0);
        assert_eq!(account.available(), Amount::ZERO);
        assert_eq!(account.total(), Amount::ZERO);
        assert_eq!(account.held(), Amount::ZERO);
        assert!(!account.locked());
    }

    #[test]
    fn test_account_ctor() {
        let account = Account::new(1);

        assert_eq!(account.id(), 1);
        assert_eq!(account.available(), Amount::ZERO);
        assert_eq!(account.total(), Amount::ZERO);
        assert_eq!(account.held(), Amount::ZERO);
        assert!(!account.locked());
    }

    #[test]
    fn test_invalid_input() {
        let mut account = Account::new(1);

        assert!(account.deposit(Amount::MIN).unwrap_err() == Error::InvalidInput);
        assert!(account.dispute(Amount::MIN).unwrap_err() == Error::InvalidInput);
        assert!(account.resolve(Amount::MIN).unwrap_err() == Error::InvalidInput);
        assert!(account.withdrawal(Amount::MIN).unwrap_err() == Error::InvalidInput);
        assert!(account.charge_back(Amount::MIN).unwrap_err() == Error::InvalidInput);
    }

    #[test]
    fn test_account_ops() {
        let mut account = Account::default();

        account.deposit(Amount::MAX).unwrap();
        assert_eq!(account.available(), Amount::MAX);
        assert_eq!(account.total(), Amount::MAX);
        assert_eq!(account.held(), Amount::ZERO);
        assert!(!account.locked());

        account.dispute(Amount::MAX).unwrap();
        assert_eq!(account.available(), Amount::ZERO);
        assert_eq!(account.total(), Amount::MAX);
        assert_eq!(account.held(), Amount::MAX);
        assert!(!account.locked());

        account.resolve(Amount::MAX).unwrap();
        assert_eq!(account.available(), Amount::MAX);
        assert_eq!(account.total(), Amount::MAX);
        assert_eq!(account.held(), Amount::ZERO);
        assert!(!account.locked());

        account.dispute(Amount::MAX).unwrap();
        assert_eq!(account.available(), Amount::ZERO);
        assert_eq!(account.total(), Amount::MAX);
        assert_eq!(account.held(), Amount::MAX);
        assert!(!account.locked());

        account.charge_back(Amount::MAX).unwrap();
        assert_eq!(account.available(), Amount::ZERO);
        assert_eq!(account.total(), Amount::ZERO);
        assert_eq!(account.held(), Amount::ZERO);
        assert!(account.locked());

        assert!(account.deposit(Amount::MAX).unwrap_err() == Error::Locked);
        assert!(account.dispute(Amount::MAX).unwrap_err() == Error::Locked);
        assert!(account.resolve(Amount::MAX).unwrap_err() == Error::Locked);
        assert!(account.withdrawal(Amount::MAX).unwrap_err() == Error::Locked);
        assert!(account.charge_back(Amount::MAX).unwrap_err() == Error::Locked);
    }

    #[test]
    fn test_serialize() {
        let mut account = Account::new(123);

        account.deposit(Amount::MAX).unwrap();

        let expected = r#"{"client":123,"available":"79228162514264337593543950335","held":"0","total":"79228162514264337593543950335","locked":false}"#;
        assert_eq!(serde_json::to_string(&account).unwrap(), expected);
    }
}
