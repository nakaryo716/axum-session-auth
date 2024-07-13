# axum-session-manager
session manager crate for axum.  
Can be used as part of authentication.

## Install
```toml
[dependencies]
axum-session-manager = "*"
```

## Usage
Only implement ```SessionManage``` Trait for your database pool or memory wraped by struct, you can use session manage system.
```rust

#[derive(Debug, Clone)]
struct Credential {
    id: i32,
    name: String,
    password: String,
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
        // insert session-id and user-data logic here 
    }

    async fn verify_session(
        &self,
        session_id: &str,
    ) -> Result<Option<Self::UserInfo>, Self::Error> {
        // query session logic here
    }

    async fn delete_session(&self, session_id: &str) {
        // delete session logic here
    }
}

```
The method of get session data(user-data) at axum::handler is use ```axum::Extention<T>```
```rust
async fn handler(Extension(user_data): Extension<UserData<Credential>>) -> impl IntoResponse {
    let try_session = user_data.get();
    match try_session {
        UserState::HaveSession(a) => {
          // do some task by using user-data
        }
        UserState::NoCookie => {
          // response StatusCode::Unauthorized
        }
        UserState::NoSession => {
          // response StatusCode::Unauthorized
    }
}

```
Please see example for more details.
