## Consistent Hashing Load Balancer
This load balancer is a reverse proxy server that utilizes consistent hashing to distribute incoming requests among backend nodes. It ensures efficient and balanced load distribution, making it suitable for scalable systems.

### Prerequisites

1. **Environment Variables:**
- Create a .env file in the root directory of your project.
- Define the backend node URLs in the .env file as follows:
```
NODE1=http://127.0.0.1:7878
NODE2=http://127.0.0.1:7879
```
2. **Port Requirements:**
- The load balancer listens on port 3000. Ensure that port 3000 is available on your system.
- Two backend nodes should be running on ports 7878 and 7879.
- A rate limiter service should be running on port 50051.


