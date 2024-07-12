use std::{fmt::Debug, pin::Pin};

use async_trait::async_trait;
use axum_core::extract::Request;
use axum_extra::extract::CookieJar;
use futures::Future;
use tower::{Layer, Service};

// Middleware cannot fail
// Whether there is a session or not is transferred to the handler(axum) side
#[derive(Debug, Clone)]
pub enum UserState<T> {
    HaveSession(T),
    NoSession,
    NoCookie,
}

// Axum handler can get UserState by using ```Extention```
#[derive(Debug, Clone)]
pub struct UserInfo<T>(UserState<T>);

impl<T> UserInfo<T> {
    pub fn new(user_state: UserState<T>) -> Self {
        Self(user_state)
    }
}

#[async_trait]
pub trait SessionManage: Debug + Clone {
    type SessionID;
    type UserInfo;
    type Error;

    async fn add_session(&self) -> Result<Self::SessionID, Self::Error>;
    async fn verify_session(&self, session_id: &str)
        -> Result<Option<Self::UserInfo>, Self::Error>;
    async fn delete_session(&self);
}

// ```users_state``` is the database pool where the user's information(user_name, email, password...) is kept
#[derive(Debug, Clone)]
pub struct AuthLayer<'a, P>
where
    P: SessionManage,
{
    sessions: P,
    session_id_key: &'a str,
}

impl<'a, P> AuthLayer<'a, P>
where
    P: SessionManage,
{
    pub fn new(sessions: P, session_id_key: &'a str) -> Self {
        Self {
            sessions,
            session_id_key,
        }
    }
}

// we can use middleware by using axum::routing::Router::layer
impl<'a, S, P> Layer<S> for AuthLayer<'a, P>
where
    P: SessionManage,
{
    type Service = AuthService<'a, S, P>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService::new(inner, self.sessions.clone(), self.session_id_key)
    }
}

#[derive(Debug, Clone)]
pub struct AuthService<'a, S, P>
where
    P: SessionManage,
{
    inner: S,
    sessions: P,
    session_id_key: &'a str,
}

impl<'a, S, P> AuthService<'a, S, P>
where
    P: SessionManage,
{
    fn new(inner: S, sessions: P, session_id_key: &'a str) -> Self {
        Self {
            inner,
            sessions,
            session_id_key,
        }
    }
}

impl<B, S, P> Service<Request<B>> for AuthService<'_, S, P>
where
    UserInfo<B>: Clone + Send + Sync + 'static,
    B: Clone + Send + Sync + 'static,
    S: Service<Request<B>> + Send + Clone + 'static,
    S::Future: Send + 'static,
    P: SessionManage + Send + Clone + 'static,
    <P as SessionManage>::UserInfo: Clone + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        // get cookie
        let jar = CookieJar::from_headers(req.headers());
        let cookie_value = jar
            .get(self.session_id_key)
            .map(|cookie| cookie.value().to_owned());

        let clone = self.inner.clone();
        let mut cloned_inner = std::mem::replace(&mut self.inner, clone);
        let cloned_session = self.sessions.clone();

        Box::pin(async move {
            let session_id = match cookie_value {
                Some(session_id) => session_id,
                None => {
                    req.extensions_mut().insert(UserInfo(UserState::NoCookie));
                    return cloned_inner.call(req).await;
                }
            };

            let query_result = cloned_session.verify_session(&session_id).await;

            match query_result {
                Ok(unchecked_session_data) => match unchecked_session_data {
                    Some(session_data) => {
                        req.extensions_mut()
                            .insert(UserInfo(UserState::HaveSession(session_data)));
                        cloned_inner.call(req).await
                    }
                    None => {
                        req.extensions_mut().insert(UserInfo(UserState::NoSession));
                        cloned_inner.call(req).await
                    }
                },
                Err(_e) => {
                    req.extensions_mut().insert(UserInfo(UserState::NoSession));
                    cloned_inner.call(req).await
                }
            }
        })
    }
}
