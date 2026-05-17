use reqwest::ClientBuilder;
use std::net::{SocketAddr, ToSocketAddrs};
use url::Url;

fn main() {
    let host = "example.com";
    let addr: SocketAddr = "127.0.0.1:80".parse().unwrap();
    let builder = ClientBuilder::new().resolve(host, addr);
    println!("Compiles!");
}
