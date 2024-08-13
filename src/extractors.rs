use std::collections::HashMap;

use ic_cdk::print;
use serde_json::{Error, Map, Value};
use serde_urlencoded::de;

use crate::CanisterRouterContext;

pub fn extract_form_or_json_data(cntx : &CanisterRouterContext) -> Result<Map<String, Value>, String> {
    let header_item = cntx.request.headers.iter().find(|header| {
        print(header.0.to_lowercase());
        if header.0.to_lowercase() == "content-type" {
            return true;
        } else {
            return false;
        }
    });

    if header_item.is_none() {
        return Err("Not Content-Type".to_string());
    }

    if header_item.unwrap().1.to_lowercase() == "application/x-www-form-urlencoded" {
        let map:Result<Map<String, Value>, de::Error> = serde_urlencoded::from_bytes(&cntx.request.body);

         if map.is_err() {
            let err = map.unwrap_err();
            print(err.to_string());
            return Err("()".to_string());
        }


        return Ok(map.unwrap());
       
    }

    if header_item.unwrap().1.to_lowercase() == "application/json" {
        print("Json Body");
        print(String::from_utf8(cntx.request.body.to_vec()).unwrap());
        let map:Result<Map<String, Value>, Error> = serde_json::from_slice(&cntx.request.body);


        if map.is_err() {
            let err = map.unwrap_err();
            print(err.to_string());
            return Err("()".to_string());
        }


        return Ok(map.unwrap());
    }

    Err("".to_string())
}