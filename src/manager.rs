use async_trait::async_trait;
use std::fmt::Debug;

/// Indicate User state  
/// HaveSession's```T```contain user Data
#[derive(Debug, Clone)]
pub enum UserState<T> {
    HaveSession(T),
    NoSession,
    NoCookie,
}

/// Wrapping UserState.
/// Axum handler can get UserState by using ```Extention```
#[derive(Debug, Clone)]
pub struct UserData<T: Clone>(pub UserState<T>);

/// Traits that implement session creation, confirmation, and deletion logic.  
/// This trait enable SessionManagerService to verify session automatically using ```verify_session``` method.  
#[async_trait]
pub trait SessionManage<T>: Debug + Clone {
    type SessionID: Clone + Send;
    type UserInfo: Clone + Send;
    type Error;

    async fn add_session(&self, session_data: T) -> Result<Self::SessionID, Self::Error>;
    async fn verify_session(&self, session_id: &str)
        -> Result<Option<Self::UserInfo>, Self::Error>;
    async fn delete_session(&self, session_id: &str) -> Result<(), Self::Error>;
}
