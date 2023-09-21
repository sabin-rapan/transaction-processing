use clap::Parser;
use tokio::fs::File;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

mod engine;
mod model;

/// Input for the transaction processing engine
#[derive(Parser, Debug)]
struct Args {
    /// Path to the transactions file to read
    file_path: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Args::parse();

    // Start the engine in its own task
    //
    // Unwrap on engine run as there is not much to do in case of failure
    let (tx, rx) = mpsc::channel(32);
    let token = CancellationToken::new();
    let cloned_token = token.clone();
    let engine_handle = tokio::spawn(async move {
        select! {
            _ = cloned_token.cancelled() => {}
            _ = engine::run(rx) => {}
        }
    });

    // Process and send transaction records to the engine in main thread, one by one as they
    // contain transaction ids which need to be processed in chronological order (similar to
    // receiving messages on a TCP socket; processing each transaction in it's own task would lead
    // to out of order transactions which is not the expected output of the program - though it's a
    // good testing scenario).
    let mut rdr = csv_async::AsyncReaderBuilder::new()
        .flexible(true)
        .trim(csv_async::Trim::All)
        .create_deserializer(File::open(args.file_path).await.unwrap());
    let mut records = rdr.deserialize::<model::transaction::TransactionRecord>();
    while let Some(record) = records.next().await {
        let record = record?;

        tx.send(engine::server::Command::ExecuteTransaction(record))
            .await?;
    }

    // Request the state of account balances
    let (resp_tx, resp_rx) = oneshot::channel();
    tx.send(engine::server::Command::GetAccountsState(resp_tx))
        .await?;
    let result = resp_rx.await?;

    // Fetch account records from engine state and process them fully and in order as there is not
    // use-case for partial results at this point.
    // Could be an optimization  for another day. Maybe.
    let mut wri = csv_async::AsyncSerializer::from_writer(tokio::io::stdout());
    for account_record in result {
        wri.serialize(account_record).await?;
    }
    wri.flush().await?;
    token.cancel();
    engine_handle.await?;

    Ok(())
}
