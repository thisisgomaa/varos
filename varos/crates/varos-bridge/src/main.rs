fn main() {
    match varos_bridge::cli::run(std::env::args().skip(1).collect()) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("varos-bridge: {e}");
            std::process::exit(1);
        }
    }
}
