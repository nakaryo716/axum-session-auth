use std::{
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use axum::{async_trait, response::IntoResponse, routing::get, Extension, Router};
use axum_session_auth::{AuthLayer, SessionManage, UserData, UserState};
use thiserror::Error;
use uuid::Uuid;

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
    let layer = AuthLayer::new(session_pool, "test-id", phantom);

    Router::new()
        .route("/", get(root))
        .route("/no_cookie", get(no_cookie))
        .layer(layer)
}

async fn root() -> impl IntoResponse {
    "Hello"
}

async fn no_cookie(Extension(user_data): Extension<UserData<Credential>>) -> impl IntoResponse {
    let a = user_data.get();
    match a {
        UserState::HaveSession(a) => {
            println!("{:?}", a);
            "we have".to_string()
        }
        UserState::NoCookie => {
            println!("called no cookie");
            "no_cookie".to_string()
        }
        UserState::NoSession => "no_session".to_string(),
    }
}

#[derive(Debug, Clone)]
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
                .unwrap()
                .insert(session_id.clone(), session_data);
        }
        Ok(session_id)
    }

    async fn verify_session(
        &self,
        session_id: &str,
    ) -> Result<Option<Self::UserInfo>, Self::Error> {
        let data = self.pool.read().unwrap();

        match data.get(session_id) {
            Some(user) => {
                let user = user.to_owned();
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    async fn delete_session(&self, session_id: &str) {
        self.pool.write().unwrap().remove(session_id);
    }
}
