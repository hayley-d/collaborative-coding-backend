use dotenv::dotenv;
use load_balancer::load_balancer::consistent_hashing::LoadBalancer;
use load_balancer::request::buffer_to_request;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let mut nodes: Vec<String> = get_nodes();

    // Listen on port 3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let listener: TcpListener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(_) => {
            eprintln!("Failed to bind to socket");
            std::process::exit(1);
        }
    };

    println!("Listening on http://{}", addr);

    let state: Arc<Mutex<LoadBalancer>> = Arc::new(Mutex::new(LoadBalancer::new(&mut nodes).await));

    let shutdown: Arc<Notify> = Arc::new(Notify::new());

    let shutdown_signal = shutdown.clone();

    tokio::spawn(async move {
        if let Err(_) = tokio::signal::ctrl_c().await {
            eprintln!("Failed to listen for shutdown signal");
            std::process::exit(1);
        } else {
            shutdown_signal.notify_one();
            println!("Tasks complete, server shutdown started");
            std::process::exit(0);
        }
    });

    tokio::select! {
        _ = reverse_proxy(listener,state.clone()) => {
            println!("loop ended");
        },
        _ = shutdown.notified() => {
                eprintln!("Graceful shutdown initiated");
                std::process::exit(0);
            }
    }

    Ok(())
}

async fn reverse_proxy(listener: TcpListener, state: Arc<Mutex<LoadBalancer>>) {
    loop {
        let state = state.clone();
        if let Ok((mut stream, client_address)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer: [u8; 4096] = [0; 4096];

                if let Ok(bytes_read) = stream.read(&mut buffer).await {
                    if bytes_read == 0 {
                        return;
                    }

                    println!(
                        "{}",
                        String::from_utf8(buffer[..bytes_read].to_vec()).unwrap()
                    );

                    let mut request: http::Request<Vec<u8>> = match buffer_to_request(
                        buffer[..bytes_read].to_vec(),
                        client_address.to_string(),
                        0,
                    ) {
                        Ok(request) => request,
                        Err(e) => {
                            eprintln!("Failed to parse request: {}", e);
                            send_error_response(400, &mut stream).await;
                            return;
                        }
                    };

                    // Ignore favicon.ico requests
                    if request.uri().path() == "/favicon.ico" {
                        send_error_response(404, &mut stream).await;
                        return;
                    }

                    // add the client IP address custom header
                    request
                        .headers_mut()
                        .insert("X-Client-IP", client_address.to_string().parse().unwrap());

                    let uri = request.uri().path().to_string();

                    let request: load_balancer::request::Request =
                        load_balancer::request::Request::new(
                            uri,
                            client_address.to_string(),
                            request,
                        );

                    let mut state = state.lock().await;

                    let response = match state.distribute(request).await {
                        Ok(r) => r,
                        Err(_) => "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n"
                            .to_string()
                            .into_bytes(),
                    };

                    if (stream.write_all(&response).await).is_err() {
                        eprintln!("Failed to responed to client");
                    };
                }
            });
        }
    }
}

async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    eprintln!("Shutdown signal received...");
}

fn get_nodes() -> Vec<String> {
    // Load the .env file
    dotenv().ok();

    let mut nodes: Vec<String> = Vec::new();

    for (key, value) in env::vars() {
        if key.starts_with("NODE") {
            nodes.push(value);
        }
    }

    println!("Loaded nodes");

    nodes
}
