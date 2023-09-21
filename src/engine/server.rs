#![deny(missing_docs)]
#![deny(warnings)]

use dashmap::DashMap;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver};
use tokio::sync::oneshot;

use crate::engine::handler::{Command as HandlerCommand, Handler};
use crate::engine::state::State;
use crate::model::account::{Account, Id as ClientId};
use crate::model::transaction::TransactionRecord;

/// Commands accepted by the Listener.
#[derive(Debug)]
pub enum Command {
    /// Execute a transaction.
    ExecuteTransaction(TransactionRecord),
    /// Get a view of all accounts.
    GetAccountsState(tokio::sync::oneshot::Sender<Vec<Account>>),
}

/// Waits for commands and dispatches them to handlers.
pub struct Listener {
    accounts: Arc<DashMap<ClientId, State>>,
    tx_handlers: HashMap<ClientId, mpsc::Sender<HandlerCommand>>,
    rx: Receiver<Command>,
}

impl Listener {
    pub fn new(rx: Receiver<Command>) -> Self {
        Self {
            accounts: Arc::new(DashMap::new()),
            tx_handlers: HashMap::new(),
            rx,
        }
    }

    /// Run the listener
    #[tracing::instrument(name = "Listener::run", skip_all)]
    pub async fn run(&mut self) {
        while let Some(cmd) = self.rx.recv().await {
            tracing::debug!("received cmd {:?}", cmd,);
            match cmd {
                Command::ExecuteTransaction(transaction) => {
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        self.tx_handlers.entry(transaction.client)
                    {
                        let (tx, mut rx) = mpsc::channel(32);

                        e.insert(tx);
                        self.accounts
                            .entry(transaction.client)
                            .or_insert(State::new(transaction.client));

                        let mut handler = Handler {
                            state: self.accounts.clone(),
                            account_id: transaction.client,
                        };

                        tracing::debug!("spawning new handler for client {}", transaction.client);
                        tokio::spawn(async move {
                            if let Err(err) = handler.run(&mut rx).await {
                                tracing::error!("handler error: {:?}", err);
                            }
                        });
                    }
                    if let Some(sender) = self.tx_handlers.get(&transaction.client) {
                        if let Err(e) = sender
                            .send(HandlerCommand::ExecuteTransaction(transaction))
                            .await
                        {
                            tracing::error!(
                                "unable to send transaction {:?}, err: {}",
                                transaction,
                                e
                            );
                        }
                    }
                }
                Command::GetAccountsState(resp) => {
                    tracing::debug!("get accounts state");
                    for handler in self.tx_handlers.values() {
                        let (resp_tx, resp_rx) = oneshot::channel();
                        match handler.send(HandlerCommand::Commit(resp_tx)).await {
                            Ok(_) => match resp_rx.await {
                                Ok(resp) => {
                                    if let Err(e) = resp {
                                        tracing::error!(
                                            "handler did not successfully commit, err: {:?}",
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "unable to receive commit response, err: {:?}",
                                        e
                                    );
                                }
                            },
                            Err(e) => {
                                tracing::error!("unable to send commit, err: {:?}", e);
                            }
                        }
                    }
                    self.tx_handlers.clear();
                    if let Err(e) = resp.send(
                        self.accounts
                            .clone()
                            .iter()
                            .map(|r| r.pair().1.account)
                            .collect::<Vec<Account>>(),
                    ) {
                        tracing::error!("unable to send accounts state, err: {:?}", e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::amount::Amount;
    use crate::model::transaction::{TransactionRecord, TransactionType};
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn test_engine() {
        // Start server
        let (tx, rx) = mpsc::channel(32);
        let mut listener = Listener::new(rx);
        tokio::spawn(async move { listener.run().await });

        // Send transactions
        let mut transactions: Vec<TransactionRecord> = Vec::new();

        for i in 1..10001 {
            transactions.push(TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client: i,
                id: i as u32,
                amount: Some(1.0),
            })
        }

        for transaction in transactions {
            tx.send(Command::ExecuteTransaction(transaction))
                .await
                .unwrap();
        }

        // Request the state of account balances
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Command::GetAccountsState(resp_tx)).await.unwrap();
        let result = resp_rx.await.unwrap();

        assert_eq!(result.len(), 10000);
        assert!(result
            .iter()
            .all(|&acc| acc.available() == Amount::from_f64(1.0).unwrap()));
        assert!(result
            .iter()
            .all(|&acc| acc.total() == Amount::from_f64(1.0).unwrap()));
        assert!(result.iter().all(|&acc| acc.held() == Amount::ZERO));
    }
}
