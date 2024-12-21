pub mod rga {
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
    /// # Example Usage
    /// ```rust
    /// use crdt::rga::rga::RGA;
    /// use crdt::S4Vector;
    ///
    /// let mut rga = RGA::new(1, 1);  // Create a new RGA instance.
    ///
    /// // Insert a value at the start.
    /// let s4_a = rga.local_insert("A".to_string(), None, None).unwrap().s4vector;
    ///
    /// // Insert another value after "A".
    /// let s4_b = rga.local_insert("B".to_string(), Some(s4_a.clone()), None).unwrap().s4vector;
    ///
    /// // Delete the first value.
    /// rga.local_delete(s4_a.clone()).unwrap();
    ///
    /// // Read the current state.
    /// let result = rga.read();
    /// assert_eq!(result, vec!["B".to_string()]);
    /// ```
    use crate::S4Vector;
    use serde::{Deserialize, Serialize};
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::rc::Rc;
    #[allow(dead_code)]

    /// Represents a node in the RGA, containing the actual data and metadata for traversal and consistency.    
    #[derive(Debug, Clone)]
    pub struct Node {
        /// The value of the node.
        pub value: String,
        /// The unique identifier for the node based on S4Vector
        pub s4vector: S4Vector,
        /// Indicates whether the node has been logically deleted.
        pub tombstone: bool,
        /// The `S4Vector` of the left neighbor
        pub left: Option<S4Vector>,
        /// The `S4Vector` of the right neighbor
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
    #[derive(Debug, Clone)]
    struct Operation {
        operation: OperationType,
        s4vector: S4Vector,
        value: Option<String>, //Optional for deletes
        left: Option<S4Vector>,
        right: Option<S4Vector>,
    }

    /// Represents the RGA structure, which is a distributed data structure
    /// supporting concurrent operations and eventual consistency.
    #[derive(Debug)]
    pub struct RGA {
        /// The head of the linked list.
        head: Option<S4Vector>,
        /// Maps `S4Vector` identifiers to `Node` instances.
        hash_map: HashMap<S4Vector, Rc<RefCell<Node>>>,
        /// A Buffer for out-of-order operations.
        buffer: VecDeque<Operation>,
        /// The current session ID.
        session_id: u64,
        /// The site ID for the current replica.
        site_id: u64,
        /// The local logical clock.
        local_sequence: u64,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum OperationError {
        #[error("Failed to perform operation, dependancies have not been met")]
        DependancyError,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct BroadcastOperation {
        pub operation: OperationType,
        pub s4vector: S4Vector,
        pub value: Option<String>,
        pub left: Option<S4Vector>,
        pub right: Option<S4Vector>,
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

        fn insert_into_list(&mut self, node: Rc<RefCell<Node>>) -> Rc<RefCell<Node>> {
            let left: Option<S4Vector> = node.borrow().left.clone();

            if let Some(left) = left {
                let mut current: S4Vector = left.clone();
                while let Some(node) = self.hash_map.get(&current) {
                    if let Some(next_s4) = &node.borrow_mut().right {
                        if next_s4 > &node.borrow().s4vector {
                            break;
                        }
                        current = next_s4.clone();
                    } else {
                        break;
                    }
                }

                if let Some(other) = self.hash_map.get(&current) {
                    node.borrow_mut().right = other.borrow().right;
                    other.borrow_mut().right = Some(node.borrow().s4vector);
                }
            }

            if self.head.is_none() {
                self.head = Some(node.borrow().s4vector);
            } else if left.is_none() {
                self.head = Some(node.borrow().s4vector);
            }

            return Rc::clone(&node);
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
        /// rga.local_insert("A".to_string(), None, None).unwrap();
        /// ```
        pub fn local_insert(
            &mut self,
            value: String,
            left: Option<S4Vector>,
            right: Option<S4Vector>,
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
            let new_node: Rc<RefCell<Node>> = Rc::new(RefCell::new(new_node));
            let node: Rc<RefCell<Node>> = self.insert_into_list(new_node);

            // Insert into the hash table
            self.hash_map
                .insert(node.borrow().s4vector, Rc::clone(&node));

            self.apply_buffered_operations();

            return Ok(BroadcastOperation {
                operation: OperationType::Insert,
                s4vector: node.borrow().s4vector,
                value: Some(node.borrow().value.clone()),
                left: node.borrow().left.clone(),
                right: node.borrow().right.clone(),
            });
        }

        /// Marks a node as logically deleted.
        ///
        /// # Parameters
        /// - `s4vector`: The unique identifier of the node to delete.
        ///
        /// # Returns
        /// `Ok(())` if the deletion is successful, otherwise an error message.
        pub fn local_delete(
            &mut self,
            s4vector: S4Vector,
        ) -> Result<BroadcastOperation, OperationError> {
            let node: Rc<RefCell<Node>> = match self.hash_map.get(&s4vector) {
                Some(node) => Rc::clone(&node),
                None => {
                    self.buffer.push_back(Operation {
                        operation: OperationType::Delete,
                        s4vector,
                        value: None,
                        left: None,
                        right: None,
                    });
                    return Err(OperationError::DependancyError);
                }
            };

            node.borrow_mut().tombstone = true;

            self.apply_buffered_operations();

            return Ok(BroadcastOperation {
                operation: OperationType::Delete,
                s4vector: node.borrow().s4vector,
                value: None,
                left: node.borrow().left.clone(),
                right: node.borrow().right.clone(),
            });
        }

        /// Marks a node as logically deleted.
        ///
        /// # Parameters
        /// - `s4vector`: The unique identifier of the node to delete.
        ///
        /// # Returns
        /// `Ok(())` if the deletion is successful, otherwise an error message.
        pub fn local_update(
            &mut self,
            s4vector: S4Vector,
            value: String,
        ) -> Result<BroadcastOperation, OperationError> {
            let node: Rc<RefCell<Node>> = match &self.hash_map.get(&s4vector) {
                Some(node) => Rc::clone(node),
                None => {
                    self.buffer.push_back(Operation {
                        operation: OperationType::Update,
                        s4vector,
                        value: Some(value),
                        left: None,
                        right: None,
                    });
                    return Err(OperationError::DependancyError);
                }
            };
            if !node.borrow().tombstone {
                node.borrow_mut().value = value;
            }
            self.apply_buffered_operations();

            return Ok(BroadcastOperation {
                operation: OperationType::Update,
                s4vector,
                value: Some(node.borrow().value.clone()),
                left: node.borrow().left.clone(),
                right: node.borrow().right.clone(),
            });
        }

        /// Remote operation to add a new element at a position based on a provided UID
        /// This operation updates the RGA to ensure eventual consistency
        pub fn remote_insert(
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
            let new_node: Rc<RefCell<Node>> = Rc::new(RefCell::new(new_node));
            let node: Rc<RefCell<Node>> = self.insert_into_list(new_node);

            self.hash_map
                .insert(node.borrow().s4vector, Rc::clone(&node));
            self.apply_buffered_operations();
        }

        /// Remote operation to remove an ekement given the UID
        /// This operation updates the RGA to ensure eventual consistency
        pub fn remote_delete(&mut self, s4vector: S4Vector) {
            let node: Rc<RefCell<Node>> = match self.hash_map.get(&s4vector) {
                Some(node) => Rc::clone(&node),
                None => {
                    // The values has not been added yet
                    return;
                }
            };
            node.borrow_mut().tombstone = true;
            self.apply_buffered_operations();
        }

        /// Remote operation to update an element
        /// This operation updates the RGA to ensure eventual consistency
        pub fn remote_update(&mut self, s4vector: S4Vector, value: String) {
            let node: Rc<RefCell<Node>> = Rc::clone(&self.hash_map[&s4vector]);
            if !node.borrow().tombstone {
                node.borrow_mut().value = value;
            }
            self.apply_buffered_operations();
        }

        /// Reads the current state of the RGA, skipping tombstoned nodes.
        ///
        /// # Returns
        /// A vector of strings representing the current sequence.
        pub fn read(&self) -> Vec<String> {
            let mut result: Vec<String> = Vec::new();
            let mut current: Option<S4Vector> = self.head;

            while let Some(current_s4) = current {
                if let Some(node) = self.hash_map.get(&current_s4) {
                    if !node.borrow().tombstone {
                        result.push(node.borrow().value.clone());
                    }

                    current = node.borrow().right;
                } else {
                    break;
                }
            }
            return result;
        }

        pub fn apply_buffered_operations(&mut self) {
            let mut buffer: VecDeque<Operation> = self.buffer.clone();

            buffer.retain(|op| {
                if let Some(left) = &op.left.clone() {
                    if !self.hash_map.contains_key(left) {
                        return true;
                    }
                }

                match op.operation {
                    OperationType::Insert => {
                        if let Some(value) = &op.value {
                            self.remote_insert(value.clone(), op.s4vector, op.left, op.right);
                        }
                    }
                    OperationType::Update => {
                        if let Some(value) = &op.value {
                            self.remote_update(op.s4vector, value.to_string());
                        }
                    }
                    OperationType::Delete => {
                        self.remote_delete(op.s4vector);
                    }
                }

                false
            });

            self.buffer = buffer;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_insert() {
            let mut rga = RGA::new(1, 1);
            let result = rga.local_insert("A".to_string(), None, None);
            assert!(result.is_ok());
            assert_eq!(rga.hash_map.len(), 1);
        }

        #[test]
        fn test_delete() {
            let mut rga = RGA::new(1, 1);
            let s4 = rga
                .local_insert("A".to_string(), None, None)
                .unwrap()
                .s4vector;
            let result = rga.local_delete(s4.clone());
            assert!(result.is_ok());
            assert!(rga.hash_map[&s4].borrow().tombstone);
        }

        #[test]
        fn test_update() {
            let mut rga = RGA::new(1, 1);
            let s4 = rga
                .local_insert("A".to_string(), None, None)
                .unwrap()
                .s4vector;
            let result = rga.local_update(s4.clone(), "B".to_string());
            assert!(result.is_ok());
            assert_eq!(rga.hash_map[&s4].borrow().value, "B".to_string());
        }

        #[test]
        fn test_read() {
            let mut rga = RGA::new(1, 1);
            rga.local_insert("A".to_string(), None, None).unwrap();
            let s4 = rga.head.unwrap();
            rga.local_insert("B".to_string(), Some(s4), None).unwrap();
            rga.local_delete(s4).unwrap();

            let result = rga.read();
            assert_eq!(result, vec!["B".to_string()]);
        }
    }
}
