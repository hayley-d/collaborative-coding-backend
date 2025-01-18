use serde::{Deserialize, Serialize};

/// `S4Vector` is a structure representing an operation in a distributed system. It ensures
/// causal consistency and deterministic ordering for collaborative applications, particularly
/// for CRDTs (Conflict-free Replicated Data Types) like the Replicated Growable Array (RGA).
///
/// Each `S4Vector` contains metadata that uniquely identifies operations in a distributed
/// multi-site, multi-session system.
///
/// # Fields
/// - `ssn`: Session ID, ensuring global uniqueness of operations within a session.
/// - `sum`: Logical clock value used for ordering operations relative to others.
/// - `sid`: Site ID, identifying the replica where the operation originated.
/// - `seq`: Sequence number, providing a local logical clock increment.
///
/// # Example
/// ```
/// use crdt::S4Vector;
/// let current_session: u64 = 1; // Session ID
/// let local_site: u64 = 42; // Replica ID
/// let mut local_sequence: u64 = 0; // Local logical clock
///
/// // Generate a base S4Vector
/// let s4_1 = S4Vector::generate(None, None, current_session, local_site, &mut local_sequence);
/// println!("S4Vector 1: {:?}", s4_1);
///
/// // Generate a new S4Vector based on s4_1
/// let s4_2 = S4Vector::generate(Some(&s4_1), None, current_session, local_site, &mut local_sequence);
/// println!("S4Vector 2: {:?}", s4_2);
///
/// assert!(s4_1 < s4_2); // Demonstrates correct ordering
/// ```
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct S4Vector {
    /// Session ID, ensuring global uniqueness of operations within a session.
    pub ssn: u64,
    /// Logical clock value used for ordering operations.
    pub sum: u64,
    /// Site ID, identifying the replica where the operation originated.
    pub sid: u64,
    /// Sequence number, providing a local logical clock increment.
    pub seq: u64,
}

impl std::hash::Hash for S4Vector {
    /// Implements hashing for `S4Vector` to allow its use in hash-based collections.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ssn.hash(state);
        self.sum.hash(state);
        self.sid.hash(state);
        self.seq.hash(state);
    }
}

impl PartialEq for S4Vector {
    // Two `S4Vector` instances are equal if all their fields match.
    fn eq(&self, other: &Self) -> bool {
        self.ssn == other.ssn
            && self.sum == other.sum
            && self.sid == other.sid
            && self.seq == other.seq
    }
}

/// Ensures two vectors are equal only iff all fields match
impl Eq for S4Vector {}

impl PartialOrd for S4Vector {
    /// Defines partial ordering for `S4Vector` using its fields.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for S4Vector {
    /// Defines total ordering for `S4Vector` using its fields in the following order:
    /// - `ssn` (Session ID)
    /// - `sum` (Logical clock value)
    /// - `sid` (Site ID)
    /// - `seq` (Sequence number)
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ssn
            .cmp(&other.ssn)
            .then(self.sum.cmp(&other.sum))
            .then(self.sid.cmp(&other.sid))
            .then(self.seq.cmp(&other.seq))
    }
}

impl S4Vector {
    /// Generates a new `S4Vector` based on neighboring nodes and the local logical clock.
    ///
    /// # Parameters
    /// - `left`: Optional reference to the left neighbor's `S4Vector`.
    /// - `right`: Optional reference to the right neighbor's `S4Vector`.
    /// - `current_session`: The current session ID.
    /// - `local_site`: The local site's unique ID.
    /// - `local_sequence`: A mutable reference to the local sequence number.
    ///
    /// # Returns
    /// A new `S4Vector` with calculated `sum` based on the provided neighbors.
    ///
    /// # Examples
    /// ```
    /// use crdt::S4Vector;
    /// let left = S4Vector { ssn: 1, sum: 10, sid: 1, seq: 1 };
    /// let right = S4Vector { ssn: 1, sum: 20, sid: 2, seq: 2 };
    /// let current_session = 1;
    /// let local_site = 42;
    /// let mut local_sequence = 0;
    ///
    /// let s4 = S4Vector::generate(Some(&left), Some(&right), current_session, local_site, &mut local_sequence);
    /// assert_eq!(s4.sum, 15); // Average of left and right sums
    /// ```
    pub fn generate(
        left: Option<&S4Vector>,
        right: Option<&S4Vector>,
        current_session: u64,
        local_site: u64,
        local_sequence: &mut u64,
    ) -> Self {
        *local_sequence += 1;

        let new_sum = match (left, right) {
            (Some(l), Some(r)) => (l.sum + r.sum) / 2, // average
            (Some(l), None) => l.sum + 1,              // append to the end
            (None, Some(r)) => r.sum / 2,              // Insert at start
            (None, None) => 1,                         // first element
        };

        S4Vector {
            ssn: current_session,
            sum: new_sum,
            sid: local_site,
            seq: *local_sequence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s4vector_equality() {
        let s4_1 = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };
        let s4_2 = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };
        let s4_3 = S4Vector {
            ssn: 1,
            sum: 11,
            sid: 42,
            seq: 2,
        };

        assert_eq!(s4_1, s4_2);
        assert_ne!(s4_1, s4_3);
    }

    #[test]
    fn test_s4vector_ordering() {
        let s4_1 = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };
        let s4_2 = S4Vector {
            ssn: 1,
            sum: 20,
            sid: 42,
            seq: 2,
        };

        assert!(s4_1 < s4_2);
    }

    #[test]
    fn test_s4vector_generate_no_neighbors() {
        let current_session = 1;
        let local_site = 42;
        let mut local_sequence = 0;

        let s4 = S4Vector::generate(None, None, current_session, local_site, &mut local_sequence);
        assert_eq!(s4.ssn, current_session);
        assert_eq!(s4.sum, 1);
        assert_eq!(s4.sid, local_site);
        assert_eq!(s4.seq, 1);
    }

    #[test]
    fn test_s4vector_generate_with_left_neighbor() {
        let current_session = 1;
        let local_site = 42;
        let mut local_sequence = 0;

        let left = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };

        let s4 = S4Vector::generate(
            Some(&left),
            None,
            current_session,
            local_site,
            &mut local_sequence,
        );
        assert_eq!(s4.sum, left.sum + 1);
        assert_eq!(s4.seq, 1);
    }

    #[test]
    fn test_s4vector_generate_with_neighbors() {
        let current_session = 1;
        let local_site = 42;
        let mut local_sequence = 0;

        let left = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };
        let right = S4Vector {
            ssn: 1,
            sum: 20,
            sid: 43,
            seq: 2,
        };

        let s4 = S4Vector::generate(
            Some(&left),
            Some(&right),
            current_session,
            local_site,
            &mut local_sequence,
        );
        assert_eq!(s4.sum, (left.sum + right.sum) / 2);
    }

    #[test]
    fn test_s4vector_hashing() {
        use std::collections::HashSet;

        let s4_1 = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };
        let s4_2 = S4Vector {
            ssn: 1,
            sum: 10,
            sid: 42,
            seq: 1,
        };

        let mut set = HashSet::new();
        set.insert(s4_1);

        assert!(set.contains(&s4_2));
    }

    #[test]
    fn test_s4vector_generate_with_right_neighbor() {
        let current_session = 1;
        let local_site = 42;
        let mut local_sequence = 0;

        let right = S4Vector {
            ssn: 1,
            sum: 20,
            sid: 43,
            seq: 2,
        };

        let s4 = S4Vector::generate(
            None,
            Some(&right),
            current_session,
            local_site,
            &mut local_sequence,
        );
        assert_eq!(s4.sum, right.sum / 2);
    }
}
