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

#[derive(Debug)]
pub enum Protocol {
    Http,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Http => write!(f, "HTTP/1.1"),
        }
    }
}

#[derive(Debug)]
pub struct Header {
    pub title: String,
    pub value: String,
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} : {}", self.title, self.value)
    }
}

#[derive(Debug)]
pub enum ContentType {
    Text,
    Html,
    Json,
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Text => write!(f, "text/plain"),
            ContentType::Html => write!(f, "text/html"),
            ContentType::Json => write!(f, "application/json"),
        }
    }
}

pub struct HttpRequest {
    pub request_id: i64,
    pub client_ip: String,
    pub headers: Vec<String>,
    pub body: String,
    pub method: HttpMethod,
    pub uri: String,
}

impl Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Request: {{method: {}, path: {}, request_id: {},client_ip: {}}}",
            self.method, self.uri, self.request_id, self.client_ip
        )
    }
}

impl HttpRequest {
    pub fn print(&self) {
        println!("{} New Request:", ">>");
        println!("{}{}", self.method.to_string(), self.uri);
    }

    pub fn new(buffer: &[u8], client_ip: String, request_id: i64) -> Result<HttpRequest, String> {
        // unwrap is safe as request has been parsed for any issues before this is called
        let request = String::from_utf8(buffer.to_vec()).unwrap();

        // split the request by line
        let request: Vec<&str> = request.lines().collect();

        if request.len() < 3 {
            eprintln!("Recieved invalid request");
            return Err(String::from("Invalid request"));
        }

        // get the http method from the first line
        let method: HttpMethod =
            HttpMethod::new(request[0].split_whitespace().collect::<Vec<&str>>()[0]);

        // get the uri from the first line
        let mut uri: String = request[0].split_whitespace().collect::<Vec<&str>>()[1].to_string();
        if uri == "/favicon.ico" {
            uri = "/".to_string();
        }

        // headers are the rest of the
        let mut headers: Vec<String> = Vec::with_capacity(request.len() - 1);
        let mut body: String = String::new();
        let mut flag = false;
        for line in &request[1..] {
            if line.is_empty() {
                flag = true;
                continue;
            }
            if flag {
                body.push_str(line);
            } else {
                headers.push(line.to_string());
            }
        }

        return Ok(HttpRequest {
            request_id,
            client_ip,
            headers,
            body,
            method,
            uri,
        });
    }

    pub fn is_compression_supported(&self) -> bool {
        for header in &self.headers {
            let header = header.to_lowercase();

            if header.contains("firefox") {
                return false;
            }

            if header.contains("accept-encoding") {
                if header.contains(',') {
                    // multiple compression types
                    let mut encodings: Vec<&str> =
                        header.split(", ").map(|m| m.trim()).collect::<Vec<&str>>();
                    encodings[0] = &encodings[0].split_whitespace().collect::<Vec<&str>>()[1];

                    for encoding in encodings {
                        if encoding == "gzip" || encoding.contains("gzip") {
                            return true;
                        }
                    }
                } else {
                    if header
                        .to_lowercase()
                        .split_whitespace()
                        .collect::<Vec<&str>>()[1]
                        == "gzip"
                    {
                        return true;
                    }
                }
            }
        }
        return false;
    }
}

#[derive(Debug)]
pub enum HttpCode {
    Ok,
    Created,
    BadRequest,
    Unauthorized,
    NotFound,
    MethodNotAllowed,
    RequestTimeout,
    Teapot,
    InternalServerError,
}
