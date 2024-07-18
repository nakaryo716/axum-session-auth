use std::{future::Future, marker::PhantomData, pin::Pin};

use axum_extra::extract::CookieJar;
use http::request::Request;
use tower::Service;

use crate::manager::{SessionManage, UserData, UserState};

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
    pub fn new(inner: S, sessions: P, session_id_key: &'a str, phantom: PhantomData<T>) -> Self {
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
                    let val: UserData<<P as SessionManage<T>>::UserInfo> = UserData(UserState::NoCookie);
                    req.extensions_mut()
                        .insert(val);
                    return cloned_inner.call(req).await;
                }
            };

            let query_result = cloned_session.verify_session(&session_id).await;
            match query_result {
                Ok(unchecked_session_data) => match unchecked_session_data {
                    Some(session_data) => {
                        req.extensions_mut()
                            .insert(UserData(UserState::HaveSession(session_data)));
                        return cloned_inner.call(req).await;
                    }
                    None => {
                        let val: UserData<<P as SessionManage<T>>::UserInfo> =
                            UserData(UserState::NoSession);
                        req.extensions_mut().insert(val);
                        return cloned_inner.call(req).await;
                    }
                },
                Err(_e) => {
                    let val: UserData<<P as SessionManage<T>>::UserInfo> =
                        UserData(UserState::NoSession);
                    req.extensions_mut().insert(val);
                    return cloned_inner.call(req).await;
                }
            }
        })
    }
}
