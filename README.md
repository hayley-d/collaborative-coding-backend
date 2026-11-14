# Real-Time Collaborative Coding Platform Backend

This project was used for me to explore **Conflict-Free Replicated Data Types (CRDTs)** and how they are able to explore eventual consistency.

The backend is for a **real-time collaborative coding platform** that enables multiple users to edit code simultaneously. The system uses **Conflict-Free Replicated Data Types (CRDTs)** for managing distributed edits, ensuring eventual consistency across replicas. The integration of CRDTs, specifically the **Replicated Growable Array (RGA)**, ensures deterministic ordering of operations. 

---
## Features
1. **Distributed System**: 
	- Designed to handle multiple replicas concurrently.
	- Supports eventual consistency.
	
2. **Conflict-Free Replicated Data Type (CRDT)**: 
	- Uses a **Replicated Growable Array (RGA)** CRDT to manage text nodes efficiently, ensuring conflict-free operation across distributed replicas.
	- Uses **S4Vector** identifiers to provide deterministic ordering of operations, even during concurrent edits.
	- Guarantees **eventual consistency** and conflict resolution.
	
3. **Scalability and Resilience**:
	- Designed to support multiple replicas and handle concurrent operations.
	- Built for distributed environments, focusing on minimizing network overhead and latency.

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
```

