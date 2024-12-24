# Real-Time Collaborative Coding Platform Backend

The platform enables multiple users to collaborate on code in real time. It focuses on a robust backend capable of handling high concurrency, ensuring data consistency, and resolving conflicts seamlessly. The system will use AWS to leverage scalability, reliability, and real-time capabilities. By leveraging Rust, Rocket, AWS RDS Aurora PostgreSQL, and AWS SNS, we achieve high performance, eventual consistency, and cloud-based scalability. The CRDT integration ensures seamless conflict resolution, making it an excellent foundation for distributed applications.

---
## Features
1. **Collaborative Editing**: Multiple users can work on a document concurrently with changes seamlessly merged using the CRDT.
2. **Conflict-Free Replicated Data Type (CRDT)**: Leveraging RGA (Replicated Growable Array) for eventual consistency in distributed enviroments.
3. **Event Propagation**: AWS SNS ensures real-time synchronization accross replicas.
4. **Cloud Integration**: Stores document metadata and snapshots using AWS RDS Aurora PostgreSQL.
5. **Rust Backend**: Built using Rocket for efficient, secure and aysnchronous HTTP API handling.

## Technology Stack
### 1. Conflict-Free Replicated Data Type (CRDT)
Using the **Replicated Growable Array (RGA)** model for conflict resolution:
- **Eventual Consistency:** Enables smooth collaboration without explicit locking mechanisms.
- **Decentralization:** Each replica operates independently, ensuring fault tolerance.
- **Ease of Merge:** The RGA ensures seamless integration of edits even in concurrent scenarios.

### 2. AWS RDS Aurora PostgreSQL
AWS RDS Aurora PostgreSQL is a fully managed relational database service designed for high-throughput, low-latency workloads. 
It was chosen because:
- **Transactional Integrity**: Aurora uses a fault-tolerant, distributed log-based storage system, ensuring strong ACID compliance.
- **Optimized for High Write Workloads**: With its write-optimized engine, Aurora PostgreSQL is well-suited for this project, which involves frequent updates and inserts to the `document_snapshots` and `operations` tables.
- **Scalable Reads**: Read replicas allow for horizontal scaling to handle increasing read demands.
- **Advanced SQL Features**: PostgreSQL's support for JSON fields and indexing mechanisms makes querying and organizing metadata efficient.

### **4. AWS SNS**
AWS SNS provides a robust mechanism for propagating events across distributed replicas:
- **Publish-Subscribe Model**: Ensures changes are sent to all replicas without polling.
- **Real-Time Notifications**: Propagates document operations to other nodes instantly.

### **2. Rocket**
Rocket is a web framework in Rust that complements the project’s goals:
- **Type-Safe Routing**: Compile-time guarantees prevent route mismatches.
- **Ease of Use**: Minimal boilerplate for setting up APIs.
- **Asynchronous Support**: Built-in support for `tokio` enables high-concurrency requests.
- **Security**: Sensible defaults for headers, cookies, and input validation.

---
## Database Design
The project uses a relational database design optimized for frequent updates and inserts. The schema ensures efficient querying and robust data integrity.

### 1. Documents Table
The documents table stores metadata about each document:
```sql
CREATE TABLE documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL,
    creation_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    title TEXT
);
```
- **document_id:** Uniquely identifies each document.
- **owner_id:** References the user who created the document.
- **creation_date:** Timestamp when the document was created.
- **title:** Title for the document.

### 2. Operations Table
The operations table records all operations for the document in a log-like fashion:
```sql
CREATE TABLE operations (
    operation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL,
    ssn BIGINT NOT NULL,    -- Session ID
    sum BIGINT NOT NULL,    -- Logical clock value
    sid BIGINT NOT NULL,    -- Site ID
    seq BIGINT NOT NULL,    -- Sequence number
    value TEXT,             -- Value of the node (optional for delete)
    tombstone BOOLEAN DEFAULT FALSE, -- Logical deletion
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```
- **operation_id:** Unique identifier for each operation.
- **document_id:** Links the operation to a specific document.
- **ssn, sum, sid, seq:** Provide order and context for operations in the distributed system.
- **value:** Represents the inserted or modified text value.
- **tombstone:** Indicates logical deletion of an element.
- **timestamp:** Captures the time of the operation.

### 3. Document Snapshots Table
The document_snapshots table maintains a history of document states for quick reconstruction and auditing:
```sql
CREATE TABLE document_snapshots (
    document_id UUID NOT NULL,
    ssn BIGINT NOT NULL,    -- Session ID
    sum BIGINT NOT NULL,    -- Logical clock value
    sid BIGINT NOT NULL,    -- Site ID
    seq BIGINT NOT NULL,    -- Sequence number
    value TEXT,             -- Value of the node (optional for delete)
    tombstone BOOLEAN DEFAULT FALSE -- Logical deletion
);
```
- **document_id:** Links the snapshot to a specific document.
- **ssn, sum, sid, seq:** Provide a sorted representation of the document's state.
- **value:** Represents the content of the snapshot.
- **tombstone:** Tracks logically deleted elements for CRDT purposes.
---
## Architecture Overview

1. **Client-Server Communication**:
   - Clients interact with the server via a RESTful API built using Rocket.
   - Operations such as creating, updating, and fetching documents are handled efficiently.

2. **Database Schema**:
   - **`document` Table**: Stores metadata about documents (ID, title, creation date, owner).
   - **`document_snapshots` Table**: Maintains a history of document states, sorted by RGA vectors.
   - **`operations` Table**: Tracks individual edit operations for CRDT-based merging.

3. **AWS SNS Integration**:
   - Notifications propagate operations to other replicas.
   - Remote replicas listen to SNS topics and integrate changes locally.

4. **Replication Logic**:
   - Uses RGA-based operations to reconcile conflicting edits in distributed nodes.

5. **Asynchronous Processing**:
   - Rust’s async/await ensures non-blocking handling of database queries, network requests, and SNS notifications.

---
## Setup Instructions

### **1. Prerequisites**
- Install [Rust](https://www.rust-lang.org/tools/install).
- Configure AWS CLI with credentials for RDS and SNS access.
- Set up an AWS RDS Aurora PostgreSQL instance.
- Create an SNS topic and configure permissions for subscribers.

### **2. Environment Variables**
Create a `.env` file with the following:
```env
DATABASE_URL=postgres://<database-user>:<database-password>@<database-host>:<port>/<database-name>
AWS_REGION=<region>
DB_HOST=<database-host>
DB_USER=<database-user>
DB_PSW=<database-password>
DB_NAME=<database-name>
AWS_REGION=<region>
SNS_TOPIC=<sns-topic-arn>
REPLICA_ID=<replica-id>
SSN_ID=<session-id>



