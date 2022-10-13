use std::io;
use zero2prod::run;

#[tokio::main]
async fn main() -> io::Result<()> {
    run()?.await
}
