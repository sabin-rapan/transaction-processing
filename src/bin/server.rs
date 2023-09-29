use dashmap::DashMap;
use transaction_processing::{DEFAULT_PORT, AccountId, Account, TransactionRecord};

use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::get, routing::post, Json,
    Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Parser, Debug)]
struct Cli {
    #[clap(long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() {
    set_up_logging();
    let cli = Cli::parse();
    let port = cli.port.unwrap_or(DEFAULT_PORT);
    let state = AppState {
        data: Arc::new(DashMap::new()),
    };
    let app = Router::new()
        .route("/accounts", get(accounts))
        .route("/", post(process_transaction))
        .with_state(state);

    axum::Server::bind(&format!("0.0.0.0:{}", port).parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap()
}

async fn accounts(State(state): State<AppState>) -> Json<Vec<Account>> {
    Json(state.data.iter().map(|r| r.pair().1.to_owned()).collect())
}

async fn process_transaction(
    State(state): State<AppState>,
    Json(payload): Json<TransactionRecord>,
) -> impl IntoResponse {
    state.data.insert(payload.client, Account::new(payload.client));

    StatusCode::OK
}

fn set_up_logging() {
    tracing_subscriber::fmt::try_init().unwrap()
}

#[derive(Deserialize, Serialize)]
struct Transaction {
    name: String,
}

#[derive(Clone)]
struct AppState {
    data: Arc<DashMap<AccountId, Account>>,
}
