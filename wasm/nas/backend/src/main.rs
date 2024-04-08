mod db;
mod error;
mod handler;

use warp::{http::StatusCode, Filter};
use warp::{Rejection};
use mobc::{Connection, Pool};
use mobc_postgres::{tokio_postgres, PgConnectionManager};
use tokio_postgres::NoTls;
use std::convert::Infallible;

type DBCon = Connection<PgConnectionManager<NoTls>>;
type DBPool = Pool<PgConnectionManager<NoTls>>;

type Result<T> = std::result::Result<T, warp::Rejection>;
impl warp::reject::Reject for Error {}

#[tokio::main]
async fn main() {
    let db_pool = db::create_pool().expect("database pool can not be created");

    db::init_db(&db_pool)
        .await
        .expect("database cannot be initialized");

    let health_route = warp::path!("health")
        .and(with_db(db_pool.clone()))
        .and_then(handler::health_handler);
    let routes = health_route
        .with(warp::cors().allow_any_origin())
        .recover(error::handle_rejection);
    warp::serve(routes).run(([127.0.0.1], 8000)).await;
}

fn with_db(db_pool: DBPool) -> impl Filter<Extract = (DBPool,), Error = Infallible> + Clone {
    warp::any().map(move || db_pool.clone())
}
