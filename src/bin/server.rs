use blaze_db::prelude;

#[tokio::main]
async fn main() {
    println!("Starting Blaze-DB HTTP Server...");
    prelude::start_server().await;
}
