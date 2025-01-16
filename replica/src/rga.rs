pub mod rga {
    use rocket::tokio::sync::RwLock;
    use uuid::{uuid, Uuid};

    /// The `RGA` module implements a Replicated Growable Array (RGA),
    /// a Conflict-free Replicated Data Type (CRDT) designed for distributed systems.
    /// This data structure supports concurrent operations such as insertions,
    /// deletions, and updates while ensuring eventual consistency and deterministic
    /// conflict resolution across multiple replicas.
    ///
    /// # Key Features
    /// - **Distributed Collaboration**: Designed for systems with concurrent updates,
    ///   such as collaborative editing tools.
    /// - **Eventual Consistency**: Ensures all replicas converge to the same state
    ///   without the need for centralized coordination.
    /// - **Efficient Buffering**: Handles out-of-order operations with a buffering
    ///   mechanism that resolves dependencies dynamically.
    ///
    /// # Example
    /// ```rust
    /// use crdt::rga::rga::RGA;
    /// use crdt::S4Vector;
    ///
    /// let mut rga = RGA::new(1, 1);  // Create a new RGA instance.
    ///
    /// // Insert a value at the start.
    /// let s4_a = rga.local_insert("A".to_string(), None, None).await.unwrap().s4vector();
    ///
    /// // Insert another value after "A".
    /// let s4_b = rga.local_insert("B".to_string(), Some(s4_a.clone()), None).await.unwrap().s4vector();
    ///
    /// // Delete the first value.
    /// rga.local_delete(s4_a.clone()).await.unwrap();
    ///
    /// // Read the current state.
    /// let result = rga.read().await;
    /// assert_eq!(result, vec!["B".to_string()]);
    /// ```
    use crate::{BroadcastOperation, S4Vector};
    use std::collections::{HashMap, VecDeque};
    use std::sync::Arc;
    #[allow(dead_code)]

    /// Represents a node in the RGA, containing the actual data and metadata for traversal and consistency.    
    /// `value`: The value of the node.
    /// `s4vector`: The unique identifier for the node based on S4Vector
    /// `tombstone`: Indicates whether the node has been logically deleted.
    /// `left`: The `S4Vector` of the left neighbor
    /// `right`: The `S4Vector` of the right neighbor
    #[derive(Debug, Clone)]
    pub struct Node {
        pub value: String,
        pub s4vector: S4Vector,
        pub tombstone: bool,
        pub left: Option<S4Vector>,
        pub right: Option<S4Vector>,
    }

    /// Enum representing different types of operations that can be applied to the RGA.
    #[derive(Debug, Clone)]
    pub enum OperationType {
        Insert,
        Update,
        Delete,
    }

    /// Represents an operation in the RGA.
    /// `operation`: Represents the operation being performed
    /// `s4vector`: The s4vector for the operation.
    /// `value`: The value being inserted or updated (None if a delete oepration)
    /// `tomestone`: Indicates a logical delete
    /// `left`: The s4vector on the left (if one exists)
    /// `right`: The s4vector on the right (if one exists)
    #[derive(Debug, Clone)]
    pub struct Operation {
        operation: OperationType,
        s4vector: S4Vector,
        value: Option<String>, //Optional for deletes
        tombstone: bool,
        left: Option<S4Vector>,
        right: Option<S4Vector>,
    }

    /// Represents the RGA structure, which is a distributed data structure
    /// supporting concurrent operations and eventual consistency.
    #[derive(Debug)]
    pub struct RGA {
        /// The head of the linked list.
        pub head: Option<S4Vector>,
        /// Maps `S4Vector` identifiers to `Node` instances.
        pub hash_map: HashMap<S4Vector, Arc<RwLock<Node>>>,
        /// A Buffer for out-of-order operations.
        pub buffer: VecDeque<Operation>,
        /// The current session ID.
        pub session_id: u64,
        /// The site ID for the current replica.
        pub site_id: u64,
        /// The local logical clock.
        pub local_sequence: u64,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum OperationError {
        #[error("Failed to perform operation, dependancies have not been met")]
        DependancyError,
    }

    impl Node {
        /// Creates a new `Node` instance.
        ///
        /// # Parameters
        /// - `value`: The content of the node.
        /// - `s4vector`: The unique identifier for this node.
        /// - `left`: The S4Vector of the left neighbor.
        /// - `right`: The S4Vector of the right neighbor.
        ///
        /// # Returns
        /// A new instance of `Node`.
        pub fn new(
            value: String,
            s4: S4Vector,
            left: Option<S4Vector>,
            right: Option<S4Vector>,
        ) -> Self {
            return Node {
                value,
                s4vector: s4,
                tombstone: false,
                left,
                right,
            };
        }

        pub fn create_from_existing(
            s4: S4Vector,
            value: String,
            tombstone: bool,
            left: Option<S4Vector>,
            right: Option<S4Vector>,
        ) -> Self {
            return Node {
                value,
                s4vector: s4,
                tombstone,
                left,
                right,
            };
        }
    }

    impl std::hash::Hash for Node {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.value.hash(state);
            self.s4vector.hash(state);
            self.tombstone.hash(state);
            self.left.hash(state);
            self.right.hash(state);
        }
    }

    impl PartialEq for Node {
        fn eq(&self, other: &Self) -> bool {
            return self.value == other.value
                && self.s4vector == other.s4vector
                && self.tombstone == other.tombstone
                && self.left == other.left
                && self.right == other.right;
        }
    }

    impl Eq for Node {}

    impl PartialOrd for Node {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(&other))
        }
    }
    impl Ord for Node {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            return self.s4vector.cmp(&other.s4vector);
        }
    }

    impl RGA {
        /// Creates a new instance of the RGA.
        ///
        /// # Parameters
        /// - `session_id`: The ID of the current session.
        /// - `site_id`: The unique ID for the current replica.
        ///
        /// # Returns
        /// A new instance of `RGA`.
        pub fn new(session_id: u64, site_id: u64) -> Self {
            return RGA {
                head: None,
                hash_map: HashMap::new(),
                buffer: VecDeque::new(),
                session_id,
                site_id,
                local_sequence: 0,
            };
        }

        /// Creates a RGA from a vector of Operations.
        /// Used when fetching an esisting document.
        /// # Parameters
        /// A Sorted vector of operations from the DB.
        ///
        /// # Returns
        /// A new instance of `RGA`.
        pub fn create_from(operations: Vec<Operation>, session_id: u64, site_id: u64) -> Self {
            let mut rga: RGA = RGA::new(session_id, site_id);

            let mut flag: bool = false;

            for op in operations {
                if !flag {
                    rga.head = Some(op.s4vector);
                    flag = true;
                }
                let value = match op.value {
                    Some(v) => v,
                    None => String::new(),
                };
                let node: Node =
                    Node::create_from_existing(op.s4vector, value, op.tombstone, op.left, op.right);
                rga.hash_map
                    .insert(op.s4vector, Arc::new(RwLock::new(node)));
                rga.local_sequence += 1;
            }
            return rga;
        }

        async fn insert_into_list(&mut self, node: Arc<RwLock<Node>>) -> Arc<RwLock<Node>> {
            let left: Option<S4Vector> = node.read().await.left.clone();

            if let Some(left) = left {
                let mut current: S4Vector = left.clone();
                while let Some(node) = self.hash_map.get(&current) {
                    if let Some(next_s4) = &node.read().await.right {
                        if next_s4 > &node.read().await.s4vector {
                            break;
                        }
                        current = next_s4.clone();
                    } else {
                        break;
                    }
                }

                if let Some(other) = self.hash_map.get(&current) {
                    node.write().await.right = other.read().await.right;
                    other.write().await.right = Some(node.read().await.s4vector);
                }
            }

            if self.head.is_none() {
                self.head = Some(node.read().await.s4vector);
            } else if left.is_none() {
                self.head = Some(node.read().await.s4vector);
            }

            return Arc::clone(&node);
        }

        /// Inserts a new value into the RGA.
        ///
        /// # Parameters
        /// - `value`: The value to insert.
        /// - `left`: The S4Vector of the left neighbor (if any).
        /// - `right`: The S4Vector of the right neighbor (if any).
        ///
        /// # Returns
        /// `Ok(())` if the insertion is successful, otherwise an error message.
        ///
        /// # Example
        /// ```rust
        /// use crdt::rga::rga::RGA;
        /// use crdt::S4Vector;
        /// let mut rga = RGA::new(1,1);
        /// rga.local_insert("A".to_string(), None, None)await.unwrap();
        /// ```
        pub async fn local_insert(
            &mut self,
            value: String,
            left: Option<S4Vector>,
            right: Option<S4Vector>,
            document_id: Uuid,
        ) -> Result<BroadcastOperation, OperationError> {
            let new_node: Node = match (left, right) {
                (Some(l), Some(r)) => {
                    // Generate the S4Vector
                    let new_s4: S4Vector = S4Vector::generate(
                        Some(&l),
                        Some(&r),
                        self.session_id,
                        self.site_id,
                        &mut self.local_sequence,
                    );

                    // Check if the dependensies are resolved
                    if !self.hash_map.contains_key(&l) {
                        self.buffer.push_back(Operation {
                            operation: OperationType::Insert,
                            s4vector: new_s4,
                            value: Some(value),
                            tombstone: false,
                            left,
                            right,
                        });
                        return Err(OperationError::DependancyError);
                    }

                    Node::new(value, new_s4, Some(l), Some(r))
                }
                (Some(l), None) => {
                    let new_s4: S4Vector = S4Vector::generate(
                        Some(&l),
                        None,
                        self.session_id,
                        self.site_id,
                        &mut self.local_sequence,
                    );

                    // Check if the dependensies are resolved
                    if !self.hash_map.contains_key(&l) {
                        self.buffer.push_back(Operation {
                            operation: OperationType::Insert,
                            s4vector: new_s4,
                            value: Some(value),
                            tombstone: false,
                            left,
                            right,
                        });
                        return Err(OperationError::DependancyError);
                    }

                    Node::new(value, new_s4, Some(l), None)
                }
                (None, Some(r)) => {
                    let new_s4: S4Vector = S4Vector::generate(
                        None,
                        Some(&r),
                        self.session_id,
                        self.site_id,
                        &mut self.local_sequence,
                    );

                    // Check if the dependensies are resolved
                    if !self.hash_map.contains_key(&r) {
                        self.buffer.push_back(Operation {
                            operation: OperationType::Insert,
                            s4vector: new_s4,
                            value: Some(value),
                            tombstone: false,
                            left,
                            right,
                        });
                        return Err(OperationError::DependancyError);
                    }

                    Node::new(value, new_s4, None, Some(r))
                }
                (None, None) => {
                    let new_s4: S4Vector = S4Vector::generate(
                        None,
                        None,
                        self.session_id,
                        self.site_id,
                        &mut self.local_sequence,
                    );

                    Node::new(value, new_s4, None, None)
                }
            };
            let new_node: Arc<RwLock<Node>> = Arc::new(RwLock::new(new_node));
            let node: Arc<RwLock<Node>> = self.insert_into_list(new_node).await;

            // Insert into the hash table
            self.hash_map
                .insert(node.read().await.s4vector, Arc::clone(&node));

            self.apply_buffered_operations().await;

            let node_guard = node.read().await;
            let (s4vector, value, left, right) = (
                node_guard.s4vector,
                node_guard.value.clone(),
                node_guard.left,
                node_guard.right,
            );

            return Ok(BroadcastOperation {
                operation: "Insert".to_string(),
                document_id,
                ssn: s4vector.ssn as i64,
                sum: s4vector.sum as i64,
                sid: s4vector.sid as i64,
                seq: s4vector.seq as i64,
                value: Some(value),
                left,
                right,
            });
        }

        /// Marks a node as logically deleted.
        ///
        /// # Parameters
        /// - `s4vector`: The unique identifier of the node to delete.
        ///
        /// # Returns
        /// `Ok(())` if the deletion is successful, otherwise an error message.
        pub async fn local_delete(
            &mut self,
            s4vector: S4Vector,
            document_id: Uuid,
        ) -> Result<BroadcastOperation, OperationError> {
            let node: Arc<RwLock<Node>> = match self.hash_map.get(&s4vector) {
                Some(node) => Arc::clone(&node),
                None => {
                    self.buffer.push_back(Operation {
                        operation: OperationType::Delete,
                        s4vector,
                        value: None,
                        tombstone: false,
                        left: None,
                        right: None,
                    });
                    return Err(OperationError::DependancyError);
                }
            };

            node.write().await.tombstone = true;

            self.apply_buffered_operations().await;

            let node_guard = node.read().await;
            let (s4vector, left, right) = (node_guard.s4vector, node_guard.left, node_guard.right);

            return Ok(BroadcastOperation {
                operation: "Delete".to_string(),
                document_id,
                ssn: s4vector.ssn as i64,
                sum: s4vector.sum as i64,
                sid: s4vector.sid as i64,
                seq: s4vector.seq as i64,
                value: None,
                left,
                right,
            });
        }

        /// Marks a node as logically deleted.
        ///
        /// # Parameters
        /// - `s4vector`: The unique identifier of the node to delete.
        ///
        /// # Returns
        /// `Ok(())` if the deletion is successful, otherwise an error message.
        pub async fn local_update(
            &mut self,
            s4vector: S4Vector,
            value: String,
            document_id: Uuid,
        ) -> Result<BroadcastOperation, OperationError> {
            let node: Arc<RwLock<Node>> = match &self.hash_map.get(&s4vector) {
                Some(node) => Arc::clone(node),
                None => {
                    self.buffer.push_back(Operation {
                        operation: OperationType::Update,
                        s4vector,
                        value: Some(value),
                        tombstone: false,
                        left: None,
                        right: None,
                    });
                    return Err(OperationError::DependancyError);
                }
            };
            if !node.read().await.tombstone {
                node.write().await.value = value;
            }
            self.apply_buffered_operations().await;
            let node_guard = node.read().await;
            let (s4vector, value, left, right) = (
                node_guard.s4vector,
                node_guard.value.clone(),
                node_guard.left,
                node_guard.right,
            );
            return Ok(BroadcastOperation {
                operation: "Update".to_string(),
                document_id,
                ssn: s4vector.ssn as i64,
                sum: s4vector.sum as i64,
                sid: s4vector.sid as i64,
                seq: s4vector.seq as i64,
                value: Some(value),
                left,
                right,
            });
        }

        /// Remote operation to add a new element at a position based on a provided UID
        /// This operation updates the RGA to ensure eventual consistency
        pub async fn remote_insert(
            &mut self,
            value: String,
            s4vector: S4Vector,
            left: Option<S4Vector>,
            right: Option<S4Vector>,
        ) {
            let new_node: Node = match (left, right) {
                (Some(l), Some(r)) => Node::new(value, s4vector, Some(l), Some(r)),
                (Some(l), None) => Node::new(value, s4vector, Some(l), None),
                (None, Some(r)) => Node::new(value, s4vector, None, Some(r)),
                (None, None) => Node::new(value, s4vector, None, None),
            };
            let new_node: Arc<RwLock<Node>> = Arc::new(RwLock::new(new_node));
            let node: Arc<RwLock<Node>> = self.insert_into_list(new_node).await;

            self.hash_map
                .insert(node.read().await.s4vector, Arc::clone(&node));
            let _ = Box::pin(async move {
                self.apply_buffered_operations().await;
            });
        }

        /// Remote operation to remove an ekement given the UID
        /// This operation updates the RGA to ensure eventual consistency
        pub async fn remote_delete(&mut self, s4vector: S4Vector) {
            let node: Arc<RwLock<Node>> = match self.hash_map.get(&s4vector) {
                Some(node) => Arc::clone(&node),
                None => {
                    // The values has not been added yet
                    return;
                }
            };
            node.write().await.tombstone = true;
            let _ = Box::pin(async move {
                self.apply_buffered_operations().await;
            });
        }

        /// Remote operation to update an element
        /// This operation updates the RGA to ensure eventual consistency
        pub async fn remote_update(&mut self, s4vector: S4Vector, value: String) {
            let node: Arc<RwLock<Node>> = Arc::clone(&self.hash_map[&s4vector]);
            if !node.read().await.tombstone {
                node.write().await.value = value;
            }
            let _ = Box::pin(async move {
                self.apply_buffered_operations().await;
            });
        }

        /// Reads the current state of the RGA, skipping tombstoned nodes.
        ///
        /// # Returns
        /// A vector of strings representing the current sequence.
        pub async fn read(&self) -> Vec<String> {
            let mut result: Vec<String> = Vec::new();
            let mut current: Option<S4Vector> = self.head;

            while let Some(current_s4) = current {
                if let Some(node) = self.hash_map.get(&current_s4) {
                    if !node.read().await.tombstone {
                        result.push(node.read().await.value.clone());
                    }

                    current = node.read().await.right;
                } else {
                    break;
                }
            }
            return result;
        }

        pub async fn apply_buffered_operations(&mut self) {
            let mut new_buffer: VecDeque<Operation> = VecDeque::new();

            for op in self.buffer.clone() {
                if let Some(left) = &op.left {
                    if !self.hash_map.contains_key(left) {
                        new_buffer.push_back(op);
                        continue;
                    }
                }

                match op.operation {
                    OperationType::Insert => {
                        if let Some(value) = &op.value {
                            self.remote_insert(
                                value.clone(),
                                op.s4vector,
                                op.left.clone(),
                                op.right.clone(),
                            )
                            .await;
                        }
                    }
                    OperationType::Update => {
                        if let Some(value) = &op.value {
                            self.remote_update(op.s4vector, value.to_string()).await;
                        }
                    }
                    OperationType::Delete => {
                        self.remote_delete(op.s4vector).await;
                    }
                }
            }

            self.buffer = new_buffer;
        }
    }

    #[cfg(test)]
    mod tests {
        use rocket::tokio;

        use super::*;

        #[tokio::test]
        async fn test_insert() {
            let mut rga = RGA::new(1, 1);
            let result = rga
                .local_insert(
                    "A".to_string(),
                    None,
                    None,
                    uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                )
                .await;
            assert!(result.is_ok());
            assert_eq!(rga.hash_map.len(), 1);
        }

        #[tokio::test]
        async fn test_delete() {
            let mut rga = RGA::new(1, 1);
            let s4 = rga
                .local_insert(
                    "A".to_string(),
                    None,
                    None,
                    uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                )
                .await
                .unwrap()
                .s4vector();
            let result = rga
                .local_delete(s4.clone(), uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
                .await;
            assert!(result.is_ok());
            assert!(rga.hash_map[&s4].read().await.tombstone);
        }

        #[tokio::test]
        async fn test_update() {
            let mut rga = RGA::new(1, 1);
            let s4 = rga
                .local_insert(
                    "A".to_string(),
                    None,
                    None,
                    uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                )
                .await
                .unwrap()
                .s4vector();
            let result = rga
                .local_update(
                    s4.clone(),
                    "B".to_string(),
                    uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
                )
                .await;
            assert!(result.is_ok());
            assert_eq!(rga.hash_map[&s4].read().await.value, "B".to_string());
        }

        #[tokio::test]
        async fn test_read() {
            let mut rga = RGA::new(1, 1);
            rga.local_insert(
                "A".to_string(),
                None,
                None,
                uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            )
            .await
            .unwrap();
            let s4 = rga.head.unwrap();
            rga.local_insert(
                "B".to_string(),
                Some(s4),
                None,
                uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            )
            .await
            .unwrap();
            rga.local_delete(s4, uuid!("67e55044-10b1-426f-9247-bb680e5fe0c8"))
                .await
                .unwrap();

            let result = rga.read().await;
            assert_eq!(result, vec!["B".to_string()]);
        }
    }
}
