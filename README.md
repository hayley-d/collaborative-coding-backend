# Real-Time Collaborative Coding Platform Backend

This backend powers a **real-time collaborative coding platform**, enabling multiple users to edit code simultaneously with seamless conflict resolution and high reliability. Built with a focus on scalability and resilience, the system utilizes **Conflict-Free Replicated Data Types (CRDTs)** for managing distributed edits, ensuring eventual consistency across replicas.

By leveraging **Rust** for its high performance and memory safety, along with **AWS SNS** and **AWS RDS Aurora PostgreSQL**, the platform achieves cloud-native scalability and real-time capabilities. The integration of CRDTs, specifically the **Replicated Growable Array (RGA)**, ensures deterministic ordering of operations, making it an ideal choice for distributed systems. This project showcases expertise in modern cloud-based architectures and high-performance backend development.

---
## Features
1. **Distributed System Design**: 
	- Designed to handle multiple replicas concurrently, with operations synchronized through efficient messaging.
	- Supports eventual consistency with lightweight coordination.
	- Uses an **RDS PostgreSQL database** for robust data persistence and scalability.
	
2. **Conflict-Free Replicated Data Type (CRDT)**: 
	- Leverages a **Replicated Growable Array (RGA)** CRDT to manage text nodes efficiently, ensuring conflict-free operation across distributed replicas.
	- Employs **S4Vector** identifiers to provide deterministic ordering of operations, even during concurrent edits.
	- Guarantees **eventual consistency** and smooth conflict resolution without requiring centralized coordination.
  
3. **AWS Integration**:
	- Integrates **AWS SNS** for broadcasting changes.
	- Uses an **RDS PostgreSQL database** for robust data persistence and scalability.
	
4. **Scalability and Resilience**:
	- Designed to support multiple replicas and handle high levels of concurrent operations without sacrificing performance.
	- Ensures durability through lightweight synchronization protocols and persistent storage.
	- Built for distributed environments, with a focus on minimizing network overhead and latency.
  
5. **Rust Backend**: 
	- Built using Rocket for efficient, secure and aysnchronous HTTP API handling.
	- Exposes clear and efficient RESTful endpoints for managing collaborative documents.
	- Implements robust error handling to provide meaningful feedback for invalid requests or system failures.
	- Designed to be extensible and adaptable to future feature additions or deployments in real-world distributed systems.
---

## Technology Stack
This project leverages a carefully chosen technology stack to deliver a robust, scalable, and high-performance backend for collaborative coding. Here's a breakdown of the stack and why each component was selected:

### **Conflict-Free Replicated Data Types (CRDTs)**
  - CRDTs provide a mathematically proven foundation for building distributed systems that handle concurrent operations without conflicts.
  - The **Replicated Growable Array (RGA)** ensures deterministic ordering of text nodes, enabling seamless real-time collaboration.
  - Removes the need for a centralized coordinator, enhancing scalability and fault tolerance.
  - Handles network partitions gracefully, ensuring eventual consistency.

### **AWS Services**
  - AWS offers highly reliable, scalable, and globally distributed services to meet the demands of modern applications.
  - **Amazon RDS (PostgreSQL)**: Ensures robust data persistence with features like automated backups, multi-AZ deployment, and read replicas.
  - **Amazon SNS**: Facilitates efficient broadcasting of changes to replicas, ensuring real-time synchronization.
  - Services are fully managed, reducing operational overhead.

### **PostgreSQL**
  - PostgreSQL's robust feature set, including support for JSONB, makes it a perfect choice for storing structured CRDT data like S4Vectors.
  - Its ACID compliance ensures reliable transaction handling.

### **Rocket Framework**
  - Rocket simplifies building RESTful APIs with its intuitive routing system and built-in support for JSON serialization/deserialization.
  - Strong type safety ensures error-free request handling.
  - Excellent integration with Rust's async ecosystem.

### **Rust**
  - Rust is known for its memory safety and zero-cost abstractions, making it ideal for building performance-critical applications.
  - Its concurrency model ensures efficient handling of multiple replicas and real-time updates.
  - The ecosystem provides powerful libraries like `tokio` for asynchronous programming and `rocket` for building RESTful APIs.

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



