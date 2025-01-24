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
