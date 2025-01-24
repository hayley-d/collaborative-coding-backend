pub struct Request {
    pub request_id: uuid::Uuid,
    pub client_ip: String,
    pub uri: String,
    pub request: http::Request<Vec<u8>>,
}

impl Request {
    pub fn new(mut uri: String, client_ip: String, mut request: http::Request<Vec<u8>>) -> Request {
        // get the uri from the first line
        if uri == "/favicon.ico" {
            uri = "/".to_string();
        }

        let request_id = Uuid::new_v4();

        // add the request ID to the headers
        request
            .headers_mut()
            .insert("X-Request-ID", request_id.to_string().parse().unwrap());

        Request {
            request_id,
            client_ip,
            uri,
            request,
        }
    }
}

pub fn buffer_to_request(
    buffer: Vec<u8>,
    client_ip: String,
    request_id: i64,
) -> Result<http::Request<Vec<u8>>, String> {
    let http_request: HttpRequest = match HttpRequest::new(&buffer, client_ip, request_id) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };

    println!("{http_request}");

    let body: Vec<u8> = Vec::new();
    return Ok(http::Request::builder()
        .method("GET")
        .uri("/")
        .body(body)
        .unwrap());
}

use core::str;
use std::fmt::Display;

#[derive(Debug)]
pub struct Clock {
    lamport_timestamp: i64,
}

impl Clock {
    pub fn new() -> Self {
        return Clock {
            lamport_timestamp: 0,
        };
    }
    pub fn increment_time(&mut self) -> i64 {
        let temp: i64 = self.lamport_timestamp;
        self.lamport_timestamp += 1;
        return temp;
    }
}
