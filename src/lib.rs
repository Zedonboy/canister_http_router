use core::str;
use std::{collections::HashMap, future::Future, pin::Pin};

use candid::{CandidType, Func};
use ic_cdk::{api::management_canister::http_request::HttpMethod, trap};
use matchit::{Params, Router};
use serde::{Deserialize, Serialize};
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


pub type HttpHeader = (String, String); 

#[derive(Clone)]
pub struct HttpResponseBuilder {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: ByteBuf,
    pub upgrade : Option<bool>,
    pub streaming_strategy: Option<StreamingStrategy>
}

impl<'a> HttpResponseBuilder {
    #[inline]
    pub fn new() -> Self {
        HttpResponseBuilder { status_code: 200, headers: vec![], body: ByteBuf::new(), upgrade: None, streaming_strategy: None }
    }

    pub fn set_status(mut self, code : u16) -> Self {
        self.status_code = code;
        self
    }

    pub fn set_body(mut self, data : ByteBuf) -> Self {
        self.body = data;
        self
    }

    pub fn set_upgrade(mut self, upgrade : Option<bool>) -> Self {
        self.upgrade = upgrade;
        self
    }

    pub fn set_headers(mut self, data : Vec<HttpHeader>) -> Self {
        self.headers = data;
        self
    }
    pub fn set_streaming_strategy(mut self, data : Option<StreamingStrategy>) -> Self {
        self.streaming_strategy = data;
        self
    }

    pub fn build(self) -> HttpResponse {
        HttpResponse { status_code: self.status_code, headers: self.headers, body:self.body, upgrade: self.upgrade, streaming_strategy: self.streaming_strategy }
    }

}

#[derive(CandidType, Deserialize, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: ByteBuf,
    pub upgrade : Option<bool>,
    pub streaming_strategy: Option<StreamingStrategy>
}

#[derive(CandidType, Deserialize, Clone)]
pub struct HttpRequest {
    pub method : String,
    pub url : String,
    pub headers : Vec<(String, String)>,
    pub body : ByteBuf,
    pub certificate_version: Option<u16>
}

impl HttpResponse {
    #[inline]
    pub fn builder<'a>() -> HttpResponseBuilder {
        HttpResponseBuilder::new()
    }
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


#[derive(CandidType, Deserialize, Clone)]
pub struct TokenData<T>(pub T);
// pub struct CallBackFunc(Func);

#[derive(CandidType, Deserialize, Clone)]
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


pub trait Handler {
    /// Handle a request.
    /// The handler is called for requests with a matching path and method.
    fn handle(
        &self,
        req: CanisterRouterContext,
    ) -> HttpResponse;
}

impl<F> Handler for F
where
    F: Fn(CanisterRouterContext) -> HttpResponse
{
    /// Handle a request.
    /// The handler is called for requests with a matching path and method.
    fn handle(
        &self,
        req: CanisterRouterContext,
    ) -> HttpResponse {
        self(req)
    }
    
    // fn handle(
    //     &self,
    //     req: CanisterRouterContext,
    // ) -> Pin<Box<HttpResponse> {
    //     Box::pin(self(req))
    // }
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

   

    pub fn process(&self, req : HttpRequest, call_context: CallType) -> HttpResponse {

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
        matcher.value.handle(cntx)


    }

}