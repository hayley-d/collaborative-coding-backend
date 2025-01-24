pub mod consistent_hashing {
    use crate::rate_limiter_proto::rate_limiter_client::RateLimiterClient;
    use crate::rate_limiter_proto::RateLimitRequest;
    use crate::request::Request;
    use std::collections::{BTreeMap, VecDeque};
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::timeout;
    use tonic::transport::Channel;

    const RATELIMITERADDRESS: &str = "http://127.0.0.1:50051";

    /// Node represents a replica in the distributed system.
    /// `address` is a url address for the replica
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct Node {
        pub address: String,
    }

    impl Node {
        /// Returns a new node based on the input parameters
        pub fn new(address: String) -> Self {
            Node { address }
        }
    }

    pub struct LoadBalancer {
        pub buffer: VecDeque<Request>,
        pub nodes: Vec<Node>,
        pub lamport_timestamp: u64,
        pub ring: BTreeMap<u64, String>,
    }

    impl LoadBalancer {
        pub fn increment_time(&mut self) -> u64 {
            let temp = self.lamport_timestamp;
            self.lamport_timestamp += 1;
            temp
        }

        pub async fn new(addresses: &mut Vec<String>) -> Self {
            let mut ring = BTreeMap::new();

            // gets the hash for each node
            for node in addresses.clone() {
                let hash = Self::add_node(&node);
                ring.insert(hash, node.clone());
            }

            let mut nodes: Vec<Node> = Vec::new();
            for node in addresses {
                nodes.push(Node {
                    address: node.to_string(),
                });
            }

            LoadBalancer {
                buffer: VecDeque::new(),
                nodes,
                lamport_timestamp: 0,
                ring,
            }
        }

        // calculates the hash of the node address for the ring
        pub fn add_node<T: Hash>(address: &T) -> u64 {
            let mut hasher = DefaultHasher::new();
            address.hash(&mut hasher);
            hasher.finish()
        }

        pub async fn distribute(&mut self, request: Request) -> Result<Vec<u8>, hyper::Error> {
            let rate_limit_request = RateLimitRequest {
                ip_address: request.client_ip.clone(),
                endpoint: request.uri.clone(),
                request_id: request.request_id.to_string(),
            };

            // send request to rate limiter
            let mut client: RateLimiterClient<Channel> =
                match RateLimiterClient::connect(RATELIMITERADDRESS.to_string().clone()).await {
                    Ok(c) => c,
                    Err(_) => {
                        eprintln!("Connection to rate limiter could not be esablished");
                        return Ok(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                                .to_string()
                                .into_bytes(),
                        );
                    }
                };

            let response = match timeout(
                Duration::from_millis(10),
                client.check_request(rate_limit_request),
            )
            .await
            {
                Ok(Ok(value)) => value,
                Ok(Err(_)) => {
                    return Ok(
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                            .to_string()
                            .into_bytes(),
                    );
                }
                Err(_) => {
                    return Ok(
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                            .to_string()
                            .into_bytes(),
                    );
                }
            };

            if !response.into_inner().allowed {
                return Ok(
                    "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n"
                        .to_string()
                        .into_bytes(),
                );
            }

            let node_address = match self.get_node(&request.client_ip) {
                Some(address) => address.clone(),
                _ => {
                    return Ok(
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                            .to_string()
                            .into_bytes(),
                    );
                }
            };

            self.increment_time();

            let request = match serialize_request(request.request).await {
                Ok(r) => r,
                _ => {
                    return Ok(
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                            .to_string()
                            .into_bytes(),
                    );
                }
            };

            let mut stream = match TcpStream::connect(node_address).await {
                Ok(s) => s,
                Err(_) => {
                    return Ok(
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                            .to_string()
                            .into_bytes(),
                    );
                }
            };

            if (stream.write_all(&request).await).is_err() {
                eprintln!("Failed to write to server");
                return Ok(
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                        .to_string()
                        .into_bytes(),
                );
            }

            let mut server_response = Vec::new();
            if (stream.read_to_end(&mut server_response).await).is_err() {
                eprintln!("Failed to read from server");
                return Ok(
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                        .to_string()
                        .into_bytes(),
                );
            }

            Ok(server_response)
        }

        /// Calculate the hash for a node using hasher instance
        pub fn get_node<H: Hash>(&self, node: &H) -> Option<&String> {
            let key = Self::add_node(node);

            self.ring
                .range(key..)
                .next()
                .map(|(_, node)| node)
                .or_else(|| self.ring.iter().next().map(|(_, node)| node))
        }
    }
}
