use reqwest::ClientBuilder;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
    let builder = ClientBuilder::new().resolve("example.com", addr);
    let client = builder.build().unwrap();
    println!("Built!");
}
