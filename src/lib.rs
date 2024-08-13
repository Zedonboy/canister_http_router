use core::str;
use std::{collections::HashMap, future::Future, pin::Pin};

use candid::{CandidType, Func};
use dyn_clone::{clone_trait_object, DynClone};
use ic_cdk::{api::management_canister::http_request::HttpMethod, trap};
use matchit::{Params, Router};
use serde::Deserialize;
use serde_bytes::ByteBuf;
use url::Url;
pub mod extractors;
candid::define_function!(pub CallBackFunc : (TokenData<()>) -> (StreamingCallbackHttpResponse<()>) query);

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

#[derive(CandidType, Deserialize)]
pub struct HttpHeader {
    name: String,
    value : String
}

#[derive(CandidType)]
pub struct HttpResponse {
    status_code: u16,
    headers: Vec<HttpHeader>,
    body: ByteBuf,
    upgrade : Option<bool>,
    streaming_strategy: Option<StreamingStrategy>
}

#[derive(CandidType, Deserialize)]
pub struct HttpRequest {
    method : String,
    url : String,
    headers : Vec<(String, String)>,
    body : ByteBuf,
    certificate_version: Option<u16>
}

impl HttpResponse {
    pub fn new() -> Self {
        HttpResponse { status_code: 200, headers: vec![], body: ByteBuf::new(), upgrade: None, streaming_strategy: None }
    }

    pub fn not_found() -> Self {
        HttpResponse { status_code: 404, headers: vec![], body: ByteBuf::new(), upgrade: None, streaming_strategy: None }
    }

    pub fn upgrade() -> Self {
        HttpResponse { status_code: 200, headers: vec![], body: ByteBuf::new(), upgrade: Some(true), streaming_strategy: None }
    }

    pub fn bad_request(mssg : Option<&str>) -> Self {
        HttpResponse { status_code: 400, headers: vec![], body: if mssg.is_none() {
            ByteBuf::new()
        } else {ByteBuf::from(mssg.unwrap().as_bytes())}, upgrade: None, streaming_strategy: None }
    }

    pub fn status(&mut self, status : u16) -> &mut Self {
        self.status_code = status;
        self
    }

    pub fn add_headers(&mut self, header_vec : Vec<HttpHeader>) -> &mut Self {
        self.headers.extend(header_vec);
        self
    }

    pub fn set_body(&mut self, body : ByteBuf) -> &mut Self {
        self.body = body;
        self
    }
}


#[derive(CandidType)]
pub struct TokenData<T>(pub T);
// pub struct CallBackFunc(Func);

#[derive(CandidType)]
pub enum StreamingStrategy {
    Callback {
        callback: CallBackFunc,
        token : TokenData<()>
    }
}

#[derive(CandidType)]
pub struct StreamingCallbackHttpResponse<T> {
    body : Vec<u8>,
    token : Option<T>
}

clone_trait_object!(Handler);
pub trait Handler: Send + Sync + DynClone {
    /// Handle a request.
    /// The handler is called for requests with a matching path and method.
    fn handle(
        &self,
        req: CanisterRouterContext,
    ) -> Pin<Box<dyn Future<Output = HttpResponse> + Send + Sync>>;
}

impl<F, R> Handler for F
where
    F: Fn(CanisterRouterContext) -> R + Send + Sync + DynClone,
    R: Future<Output = HttpResponse> + Send + Sync + 'static,
{
    /// Handle a request.
    /// The handler is called for requests with a matching path and method.
    fn handle(
        &self,
        req: CanisterRouterContext,
    ) -> Pin<Box<dyn Future<Output = HttpResponse> + Send + Sync>> {
        Box::pin(self(req))
    }
}



pub enum CallType {
    Query,
    Update
}

pub struct CanisterRouterContext {
    pub request : HttpRequest,
    pub params : Option<HashMap<String, String>>,
    pub call_type : CallType,
    pub query : Option<HashMap<String, String>>,
}

#[derive(Clone)]
pub struct CanisterRouter {
    _route_tree : HashMap<HttpMethod, Router<Box<dyn Handler>>>
}

impl CanisterRouter {
    pub fn new() -> Self {
        CanisterRouter {
            _route_tree : HashMap::new()
        }
    }



    pub fn get(&mut self, url : &str, handler : impl Handler + 'static) -> &mut Self {
        if !url.starts_with('/') {
            trap(format!("expect path beginning with '/', found: '{}'", url).as_str());
        }
        let _ = self._route_tree.entry(HttpMethod::GET).or_insert(Router::default()).insert(url, Box::new(handler));
        self
    }

    pub fn post(&mut self, url : &str, handler : impl Handler + 'static) -> &mut Self {
        if !url.starts_with('/') {
            trap(format!("expect path beginning with '/', found: '{}'", url).as_str());
        }
        let _ = self._route_tree.entry(HttpMethod::POST).or_insert(Router::default()).insert(url, Box::new(handler));
        self
    }

   

    pub async fn process(&self, req : HttpRequest, call_context: CallType) -> HttpResponse {

        let router_opt = match req.method.as_str() {
            "POST" => {
                let router_opt = self._route_tree.get(&HttpMethod::POST);
                router_opt
            },

            "GET" => {
                let router_opt = self._route_tree.get(&HttpMethod::GET);
                router_opt
            },

            "HEAD" => {
                let router_opt = self._route_tree.get(&HttpMethod::HEAD);
                router_opt
            },
            _ => {
                None
            }
        };

        if router_opt.is_none() {
            return HttpResponse::not_found();
        }

        let router = router_opt.unwrap();
        let uri = req.url.clone();
        let matcher_rslt = router.at(&uri);
        if matcher_rslt.is_err() {
            let mut resp = HttpResponse::not_found();
            resp.set_body(ByteBuf::from("There was no match"));
            return resp;
        }

        let matcher = matcher_rslt.unwrap();
        let url_parse = Url::parse(format!("http://example.com{}", &req.url).as_str());
        if url_parse.is_err() {
            let err = url_parse.unwrap_err();
            return HttpResponse::bad_request(Some(format!("Url is invalid: {}", err).as_str()));
        }

        let url = url_parse.unwrap();
        
        let cntx = CanisterRouterContext {
            request: req,
            params: if matcher.params.is_empty() {
                None
            } else {
                let mut _df : HashMap<String, String> = HashMap::new();
                for (key, value) in matcher.params.iter() {
                    _df.insert(key.to_string(), value.to_string());
                }

                Some(_df)
            },
            call_type: call_context,
            query : if url.query().is_none() {
                None
            } else {
                let mut _df : HashMap<String, String> = HashMap::new();
                for (k, v) in url.query_pairs() {
                    _df.insert(k.into_owned(), v.into_owned());
                }
                Some(_df)
            }
        };
        matcher.value.handle(cntx).await


    }

}