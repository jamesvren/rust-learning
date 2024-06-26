use create::{db, DBPool};
use warp::{http::StatusCode, reject, Reply, Rejection};

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

pub async fn health_handler(db_pool: DBPool) -> std::result::Result<impl Reply, Rejection> {
    let db = db::get_db_con(&db_pool)
        .await
        .map_err(|e| reject::custom(e))?;

    db.execute("SELECT 1", &[])
        .await
        .map_err(|e| reject::custom(DBQueryError(e)))?;
    Ok(StatusCode::OK)
}

pub async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    let code;
    let messsage;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "Not Found";
    } else if let Some(_) = err.find::<warp::filters::body::BodyDeserializeError>() {
        code = StatusCode::BAD_REQUEST;
        message = "Invalid Body";
    } else if let Some(e) = err.find::<Error>() {
        match e {
            Error:::DBQueryError(_) => {
                code = StatusCode::BAD_REQUEST;
                message = "Could not Execute request";
            }
            _ => {
                eprintln!("unhandled application error: {:?}", err);
                code = StatusCode::INTERNAL_SERVER_ERROR;
                message = "Internal Server Error";
            }
        }
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        code = StatusCode::METHOD_NOT_ALLOWED;
        message = "Method Not Allowed";
    } else {
        eprintln!("unhandled error: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal Server Error";
    }

    let json = warp::reply::json(&ErrorResponse {
        message: message.into(),
    })

    Ok(warp::reply::with_status(json, code))
}
