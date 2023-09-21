pub mod handler;
pub mod server;
pub mod state;

use tokio::sync::mpsc::Receiver;

/// Run the engine.
pub async fn run(rx: Receiver<server::Command>) {
    let mut listener = server::Listener::new(rx);

    listener.run().await
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
        tokio::spawn(async move {
            run(rx).await;
        });

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
            tx.send(server::Command::ExecuteTransaction(transaction))
                .await
                .unwrap();
        }

        // Request the state of account balances
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(server::Command::GetAccountsState(resp_tx))
            .await
            .unwrap();
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
