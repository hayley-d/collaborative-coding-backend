pub struct Request {
    pub request_id: uuid::Uuid,
    pub client_ip: String,
    pub uri: String,
    pub request: http::Request<Vec<u8>>,
}
