#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .unwrap();
    match client.get("https://soundcloud.com").send().await {
        Ok(r) => println!("SoundCloud status: {}", r.status()),
        Err(e) => println!("Network diagnostic: {:?}", e.without_url()),
    };
    let proxy = std::env::var("HTTPS_PROXY").ok();
    if let Some(proxy) = proxy {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .proxy(reqwest::Proxy::all(proxy).unwrap())
            .build()
            .unwrap();
        match client.get("https://soundcloud.com").send().await {
            Ok(r) => println!("Explicit proxy status: {}", r.status()),
            Err(e) => println!("Explicit proxy diagnostic: {:?}", e.without_url()),
        };
    }
}
