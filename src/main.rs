fn main() {
    if let Err(err) = unused_buddy::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
