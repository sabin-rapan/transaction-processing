use transaction_processing::{
    AccountId, TransactionId, TransactionRecord, TransactionType, DEFAULT_PORT,
};

use clap::{Parser, Subcommand};
use hyper::body::HttpBody as _;
use hyper::Client;
use hyper::{Body, Method, Request};
use std::str;
use tokio::io::{stdout, AsyncWriteExt as _};

#[derive(Parser, Debug)]
#[clap(name = "transaction-processing-cli", version, about = "Issue commands")]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(name = "hostname", long, default_value = "127.0.0.1")]
    host: String,

    #[clap(long, default_value_t = DEFAULT_PORT)]
    port: u16,
}

#[derive(Subcommand, Debug)]
enum Command {
    GetAccounts,
    ProcessTransaction {
        id: TransactionId,
        client: AccountId,
        transaction_type: String,
        amount: Option<f64>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::try_init()?;

    let cli = Cli::parse();
    let address = match cli.command {
        Command::GetAccounts => format!("http://{}:{}/accounts", cli.host, cli.port),
        Command::ProcessTransaction {
            id: _,
            client: _,
            transaction_type: _,
            amount: _,
        } => format!("http://{}:{}/", cli.host, cli.port),
    };
    let c = Client::new();
    let uri = address.parse()?;
    let mut resp = match cli.command {
        Command::GetAccounts => c.get(uri).await?,
        Command::ProcessTransaction {
            id,
            client,
            transaction_type: _,
            amount,
        } => {
            let transaction = TransactionRecord {
                id,
                client,
                transaction_type: TransactionType::Deposit,
                amount,
            };
            let req = Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&transaction)?))?;
            c.request(req).await?
        }
    };
    println!("Response: {}", resp.status());

    while let Some(chunk) = resp.body_mut().data().await {
        stdout().write_all(&chunk?).await?;
    }
    Ok(())
}
