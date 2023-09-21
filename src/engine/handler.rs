#![deny(missing_docs)]
#![deny(warnings)]

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

use crate::engine::state::{State, Transaction};
use crate::model::account::Id as AccountId;
use crate::model::transaction::TransactionRecord;

/// Error conditions that may arise when creating a new `Handler` object.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The state of the handler is invalid.
    #[error("Invalid state")]
    InvalidState,
}

/// Result of account operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Commands received by the Handler from the Listener.
#[derive(Debug)]
pub enum Command {
    /// Execute a transaction.
    ExecuteTransaction(TransactionRecord),
    /// Finish executing pending transactions and return.
    Commit(tokio::sync::oneshot::Sender<Result<()>>),
}

/// Handles transactions on a single account.
pub struct Handler {
    /// Sharded state of a single account.
    pub state: Arc<DashMap<AccountId, State>>,
    /// Account id of this handler.
    pub account_id: AccountId,
}

impl Handler {
    #[tracing::instrument(name = "Handler::run", skip_all)]
    pub async fn run(&mut self, rx: &mut Receiver<Command>) -> Result<()> {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::ExecuteTransaction(transaction_record) => {
                    if transaction_record.client != self.account_id {
                        tracing::error! {
                            %transaction_record.client, %self.account_id,
                            "received transaction for another endpoint"
                        };
                        continue;
                    }
                    match Transaction::try_from(transaction_record) {
                        Ok(transaction) => {
                            let mut state = self
                                .state
                                .get_mut(&transaction_record.client)
                                .ok_or(Error::InvalidState)?;

                            match transaction.apply(state.value_mut()) {
                                Ok(_) => {
                                    tracing::debug! {
                                        %transaction_record.client, %transaction,
                                        "success"
                                    };
                                }
                                Err(e) => {
                                    tracing::warn! {
                                        %transaction_record.client, %transaction, %e,
                                        "failure"
                                    };
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn! {
                                %transaction_record, %e,
                                "invalid transaction record"
                            };
                        }
                    }
                }
                Command::Commit(resp) => {
                    tracing::debug!("received commit");
                    if let Err(e) = resp.send(Ok(())) {
                        tracing::error!("unable to send commit response, err: {:?}", e);
                    }
                    rx.close();
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::state::TransactionMetadata;
    use crate::model::amount::Amount;
    use crate::model::transaction::TransactionType;

    use super::*;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn test_handler() {
        let client_id = 1;
        let state: Arc<DashMap<AccountId, State>> = Arc::new(DashMap::new());
        state.insert(client_id, State::new(client_id));

        let (tx, mut rx) = mpsc::channel(32);
        let mut handler = Handler {
            state: state.clone(),
            account_id: client_id,
        };

        let handle = tokio::spawn(async move {
            handler.run(&mut rx).await.unwrap();
        });
        let mut transactions = Vec::new();
        // Invalid account id
        transactions.push(TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client: client_id + 1,
            id: 1,
            amount: Some(1.0),
        });
        // Invalid deposit transaction
        transactions.push(TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client: client_id,
            id: 2,
            amount: None,
        });
        // Valid deposit
        transactions.push(TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client: client_id,
            id: 3,
            amount: Some(12.34),
        });
        for transaction in transactions {
            tx.send(Command::ExecuteTransaction(transaction))
                .await
                .unwrap();
        }
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Command::Commit(resp_tx)).await.unwrap();
        let result = resp_rx.await.unwrap();
        let _ = result.unwrap();
        handle.await.unwrap();

        assert_eq!(
            state.get(&client_id).unwrap().account.available(),
            Amount::from_f64(12.34).unwrap()
        );
        assert_eq!(
            state.get(&client_id).unwrap().account.total(),
            Amount::from_f64(12.34).unwrap()
        );
        assert_eq!(state.get(&client_id).unwrap().account.held(), Amount::ZERO);
        assert_eq!(state.get(&client_id).unwrap().account.locked(), false);
        assert_eq!(state.get(&client_id).unwrap().account.id(), client_id);
        assert_eq!(state.get(&client_id).unwrap().transaction_history.len(), 1);
        assert_eq!(
            state
                .get(&client_id)
                .unwrap()
                .transaction_history
                .get(&3)
                .unwrap(),
            &Transaction::Deposit(
                TransactionMetadata(3, client_id),
                Amount::from_f64(12.34).unwrap(),
                false
            )
        );
    }
}
