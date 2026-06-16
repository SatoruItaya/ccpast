mod util;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("ccpast {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    eprintln!("ccpast: not implemented yet");
    std::process::exit(2);
}

fn print_help() {
    println!(
        "ccpast {}\n\
         Browse Claude Code session history.\n\
         \n\
         USAGE:\n    ccpast [FLAGS]\n\
         \n\
         FLAGS:\n\
             --list           Print sessions to stdout instead of launching the TUI\n\
             -h, --help       Show this help\n\
             -V, --version    Show version",
        env!("CARGO_PKG_VERSION")
    );
}
