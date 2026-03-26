#[tokio::main]
async fn main() {
    if let Err(err) = captions::run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
