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
    }
}
