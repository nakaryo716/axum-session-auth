use std::fmt::Debug;

use async_trait::async_trait;
use axum_core::extract::Request;
use axum_extra::extract::CookieJar;
use futures::executor::block_on;
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

// SessionStore is the database pool where the cookie-value and session_id is kept
#[derive(Debug, Clone)]
pub struct SessionStore<P> {
    pool: P,
}

#[async_trait]
pub trait SessionManage: Debug + Clone {
    async fn add_session(&self);
    async fn verify_session(&self) -> String;
    async fn delete_session(&self);
}

// ```users_state``` is the database pool where the user's information(user_name, email, password...) is kept
#[derive(Debug, Clone)]
pub struct AuthLayer<T, P>
where
    P: SessionManage,
{
    users_state: T,
    sessions: P,
}

impl<T, P> AuthLayer<T, P>
where
    P: SessionManage,
{
    pub fn new(users_state: T, sessions: P) -> Self {
        Self {
            users_state,
            sessions,
        }
    }
}

// we can use middleware by using axum::routing::Router::layer
impl<S, T, P> Layer<S> for AuthLayer<T, P>
where
    T: Clone,
    P: SessionManage,
{
    type Service = AuthService<S, T, P>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService::new(inner, self.users_state.clone(), self.sessions.clone())
    }
}

#[derive(Debug, Clone)]
pub struct AuthService<S, T, P>
where
    P: SessionManage,
{
    inner: S,
    users_state: T,
    sessions: P,
}

impl<S, T, P> AuthService<S, T, P>
where
    P: SessionManage,
{
    fn new(inner: S, users_state: T, sessions: P) -> Self {
        Self {
            inner,
            users_state,
            sessions,
        }
    }
}

impl<B, S, T, P> Service<Request<B>> for AuthService<S, T, P>
where
    S: Service<Request<B>> + Send,
    P: SessionManage + Send,
    B: Send,
    T: Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        // get cookie
        let jar = CookieJar::from_headers(req.headers());
        let cookie_value = jar.get("foo").map(|cookie| cookie.value().to_owned());
        // query session
        let query_result = block_on(&mut self.sessions.verify_session());

        // add extention mutable
        // call req to routed handler
        self.inner.call(req)
    }
}
