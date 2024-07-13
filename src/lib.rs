use std::{fmt::Debug, future::Future, marker::PhantomData, pin::Pin};

use async_trait::async_trait;
use axum_core::extract::Request;
use axum_extra::extract::CookieJar;
use tower::Layer;
use tower_service::Service;

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
pub struct UserData<T: Clone>(UserState<T>);

impl<T> UserData<T>
where
    T: Clone,
{
    pub fn get(&self) -> UserState<T> {
        self.0.clone()
    }
}

#[async_trait]
pub trait SessionManage<T>: Debug + Clone {
    type SessionID: Clone + Send;
    type UserInfo: Clone + Send;
    type Error;

    async fn add_session(&self, session_data: T) -> Result<Self::SessionID, Self::Error>;
    async fn verify_session(&self, session_id: &str)
        -> Result<Option<Self::UserInfo>, Self::Error>;
    async fn delete_session(&self, session_id: &str);
}

// ```users_state``` is the database pool where the user's information(user_name, email, password...) is kept
#[derive(Debug, Clone)]
pub struct SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
    sessions: P,
    session_id_key: &'a str,
    phantome: PhantomData<T>,
}

impl<'a, P, T> SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
    pub fn new(sessions: P, session_id_key: &'a str, phantome: PhantomData<T>) -> Self {
        Self {
            sessions,
            session_id_key,
            phantome,
        }
    }
}

// we can use middleware by using axum::routing::Router::layer
impl<'a, S, P, T> Layer<S> for SessionManagerLayer<'a, P, T>
where
    P: SessionManage<T>,
{
    type Service = SessionManagerService<'a, S, P, T>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionManagerService::new(
            inner,
            self.sessions.clone(),
            self.session_id_key,
            self.phantome.clone(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct SessionManagerService<'a, S, P, T>
where
    P: SessionManage<T>,
{
    inner: S,
    sessions: P,
    session_id_key: &'a str,
    phantom: PhantomData<T>,
}

impl<'a, S, P, T> SessionManagerService<'a, S, P, T>
where
    P: SessionManage<T>,
{
    fn new(inner: S, sessions: P, session_id_key: &'a str, phantom: PhantomData<T>) -> Self {
        Self {
            inner,
            sessions,
            session_id_key,
            phantom,
        }
    }
}

impl<B, S, P, T> Service<Request<B>> for SessionManagerService<'_, S, P, T>
where
    B: Send + 'static,
    S: Service<Request<B>> + Send + Clone + 'static,
    S::Future: Send + 'static,
    P: SessionManage<T> + Send + Clone + 'static,
    T: Clone + Send + Sync + 'static,
    <P as SessionManage<T>>::UserInfo: Clone + Send + Sync + 'static,
    <P as SessionManage<T>>::Error: std::marker::Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

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
                    req.extensions_mut()
                        .insert(UserData(UserState::<T>::NoCookie));
                    println!("calle cookie err");
                    return cloned_inner.call(req).await;
                }
            };

            let query_result = cloned_session.verify_session(&session_id).await;

            match query_result {
                Ok(unchecked_session_data) => match unchecked_session_data {
                    Some(session_data) => {
                        req.extensions_mut()
                            .insert(UserData(UserState::HaveSession(session_data)));
                        println!("calle some");
                        return cloned_inner.call(req).await;
                    }
                    None => {
                        req.extensions_mut()
                            .insert(UserData(UserState::<T>::NoSession));
                        println!("calle none");
                        return cloned_inner.call(req).await;
                    }
                },
                Err(_e) => {
                    req.extensions_mut()
                        .insert(UserData(UserState::<T>::NoSession));
                    println!("calle err");
                    return cloned_inner.call(req).await;
                }
            }
        })
    }
}
