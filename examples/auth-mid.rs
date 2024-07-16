use std::{
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use axum::{
    async_trait,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::{cookie::Cookie, CookieJar};
use axum_session_manager::{SessionManage, SessionManagerLayer, UserData, UserState};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const COOKIE_KEY: &str = "test-id";
#[tokio::main]
async fn main() {
    let service = app();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, service).await.unwrap();
}

fn app() -> Router {
    let session_pool = SessionPool::new();
    let phantom = PhantomData::default();
    let layer = SessionManagerLayer::new(session_pool.clone(), COOKIE_KEY, phantom);

    Router::new()
        .route("/", get(root))
        .route("/login", post(login))
        .route("/session/data", get(get_session_data))
        .layer(layer)
        .with_state(session_pool)
}

async fn root() -> impl IntoResponse {
    "Hello"
}

async fn get_session_data(
    Extension(user_data): Extension<UserData<Credential>>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let user_state = user_data.0;
    match user_state {
        UserState::HaveSession(a) => Ok((StatusCode::OK, Json(a))),
        UserState::NoCookie => Err((StatusCode::UNAUTHORIZED, "no cookie")),
        UserState::NoSession => Err((StatusCode::UNAUTHORIZED, "you need login")),
    }
}

async fn login(
    State(session_pool): State<SessionPool>,
    jar: CookieJar,
    Json(payload): Json<Credential>,
) -> Result<impl IntoResponse, StatusCode> {
    let session_id = session_pool
        .add_session(payload)
        .await
        .map_err(|_e| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cookie = Cookie::new(COOKIE_KEY, session_id);

    Ok((StatusCode::OK, jar.add(cookie)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Credential {
    id: i32,
    name: String,
    password: String,
}

#[derive(Debug, Clone)]
struct UserPool {
    pool: Arc<RwLock<HashMap<i32, Credential>>>,
}

impl UserPool {
    fn new() -> Self {
        Self {
            pool: Arc::default(),
        }
    }
}

#[derive(Debug, Clone)]
struct SessionPool {
    pool: Arc<RwLock<HashMap<String, Credential>>>,
}

impl SessionPool {
    fn new() -> Self {
        Self {
            pool: Arc::default(),
        }
    }
}

#[derive(Debug, Clone, Error)]
enum ServerError {
    #[error("unexpected error")]
    Unexpect,
}

#[async_trait]
impl SessionManage<Credential> for SessionPool {
    type SessionID = String;
    type UserInfo = Credential;
    type Error = ServerError;

    async fn add_session(&self, session_data: Credential) -> Result<Self::SessionID, Self::Error> {
        let session_id = Uuid::new_v4().to_string();
        {
            let _ = &self
                .pool
                .write()
                .map_err(|_e| ServerError::Unexpect)?
                .insert(session_id.clone(), session_data);
        }
        Ok(session_id)
    }

    async fn verify_session(
        &self,
        session_id: &str,
    ) -> Result<Option<Self::UserInfo>, Self::Error> {
        let data = self.pool.read().map_err(|_e| ServerError::Unexpect)?.to_owned();

        match data.get(session_id) {
            Some(user) => {
                let user = user.to_owned();
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), Self::Error> {
        self.pool
            .write()
            .map_err(|_e| ServerError::Unexpect)?
            .remove(session_id);
        Ok(())
    }
}
