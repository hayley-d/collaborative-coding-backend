# Real-Time Collaborative Coding Platform Backend
The platform enables multiple users to collaborate on code in real time. It focuses on a robust backend capable of handling high concurrency, ensuring data consistency, and resolving conflicts seamlessly. The system will use AWS to leverage scalability, reliability, and real-time capabilities.

## 1. **Core Features**
#### 1.1 Live Collaboration
Users can edit code collaboratively in real time.
Edits are synchronized across all participants instantly.
Implement features like syntax highlighting and code folding on the client-side (optional).
#### 1.2 Conflict Resolution
Handle simultaneous edits from multiple users using algorithms like Operational Transformation (OT) or Conflict-free Replicated Data Types (CRDTs).
Provide a real-time editing experience without data loss or corruption.
#### 1.3 Notifications
Notify users of changes (e.g., new participants joining, edits, or comments).
Optional: Integrate chat functionality for real-time communication between collaborators.
#### 1.4 Session Management
Allow users to create, join, or leave collaborative sessions.
Session state is preserved until explicitly terminated.
#### 1.5 Versioning
Maintain version history for each document, allowing users to revert to previous versions if needed.

## 2.2 AWS Services
| Service	| Purpose| 
| -------| -------| 
| API Gateway	| WebSocket API for real-time communication between clients and the backend.| 
| Lambda	| Event-driven serverless functions for conflict resolution, notifications, etc.| 
| DynamoDB	| Low-latency database for storing documents and metadata.| 
| SQS/EventBridge	| Manage event-driven workflows for notifications and background tasks.| 
| Cognito	| User authentication and authorization.| 
| CloudWatch	| Monitor performance and log events.| 
| S3	(Optional) | Store large files or assets (e.g., user-uploaded documents).| 


## Backend Workflow
#### 3.1 Client Connection
WebSocket Setup:
Clients establish a connection to the backend via AWS API Gateway (WebSocket API).
Each client is assigned a unique session ID and user ID upon connection.
#### 3.2 Collaborative Editing
Event Handling:

When a user types, an "Edit" event is sent to the backend with details such as:
Document ID.
User ID.
Edit operation (e.g., "Insert 'x' at position 10").
Backend broadcasts the event to all other participants in the session.
Conflict Resolution:

Lambda functions resolve conflicts using OT or CRDT algorithms.
Changes are applied to the document state and saved in DynamoDB.
#### 3.3 Real-Time Notifications
Event Dispatch:
Notifications (e.g., "User X joined the session") are pushed to connected clients using WebSocket.
SQS/EventBridge ensures event delivery and retries for failure.
#### 3.4 Persistence and Versioning
Database Writes:

Edits are periodically saved to DynamoDB for persistence.
Each change is versioned for traceability.
Version Retrieval:

Users can request previous versions, which the backend fetches from DynamoDB.

## Choices
### Rocket Framework
I chose to work with rocket as it works with a fully asynchronous core and all asynchronous tasks are multiplexed on a configurable number of worker threads.

## Overall TODO
- RESTful API to handle document updates and synchronization
- Database setup for persistence of documents and CRDT states.
- Infrastructure: AWS lambda, API gateway and S3

## TODO
- Define functionality to manage and discover replicas in the network
- Secure routes to prevent unauthorized access to sensitive information
- Ensure all API routes handle edge cases (add unit tests and integration tests)
- Integrate a logging crate to debug and monitor operaions
- Add lambda calls to aws for database entries

- DB setup
use MySQL version of Aurora and confugure a connection pooler to handle concurrent connections.
use sqlx (crate) for asynchronous database queries (or tokio-postgres for postgres)
use AWS api gateway to deliver real-time updates to clients.

- communication
use AWS SNS/AWS SQS it acts as a central hub to broadcast messages and delivers to other replicas via HTTP or lambda


Steps: 
1. Setup DB
2. Build and Test server comminication
3. Write unit tests
4. Set up replica Broadcasting

tables: 
- Operations Table: records all operations like a log
- 

## Schemas

The documents table stores the metadata about each document.

CREATE TABLE documents (
    document_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL,
    creation_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    title TEXT
);

The operations table records all operations for the document in a log-like fashion.
The ssn,sum,sid and seq components are used to order the operations when reconstructing the document.

CREATE TABLE operations (
    operation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL,
    ssn BIGINT NOT NULL,    -- Session ID
    sum BIGINT NOT NULL,    -- Logical clock value
    sid BIGINT NOT NULL,    -- Site ID
    seq BIGINT NOT NULL,    -- Sequence number
    value TEXT,             -- Value of the node (optional for delete)
    tombstone BOOLEAN DEFAULT FALSE, -- Logical deletion
    left_s4 JSONB,          -- JSON representation of left S4Vector
    right_s4 JSONB,         -- JSON representation of right S4Vector
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);


Example Query: 
Query the operations table for a specific document ID.

```SQL
SELECT value
FROM operations
WHERE document_id = '<document_id>'
ORDER BY ssn, sum, sid, seq;
```


