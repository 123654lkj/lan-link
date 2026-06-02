//! mDNS 服务发现
//! 通过局域网多播广播服务，让客户端自动发现 daemon 地址。

//! mDNS-based peer discovery.
//!
//! Broadcasts _lan-link._udp service on the local network.

pub async fn run(port: u16) {
    tracing::info!("mDNS discovery started (port {})", port);
    // TODO: implement mDNS via mdns-sd or simple-mdns crate
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
}
