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
