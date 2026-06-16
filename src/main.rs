use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

mod parser;
mod scan;
mod session;
mod util;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("ccpast {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return ExitCode::SUCCESS;
    }

    let force_list = args.iter().any(|a| a == "--list");
    let tty = io::stdout().is_terminal();

    if force_list || !tty {
        return run_list();
    }

    // TUI path is added in a later task; for now, fall through to --list.
    run_list()
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

fn run_list() -> ExitCode {
    let Some(root) = scan::projects_root() else {
        eprintln!("ccpast: cannot determine $HOME");
        return ExitCode::from(2);
    };

    let mut metas: Vec<_> = scan::list_session_files(&root)
        .iter()
        .filter_map(|p| session::extract_meta(p))
        .collect();
    metas.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

    let stdout = io::stdout();
    let mut out = stdout.lock();
    if metas.is_empty() {
        let _ = writeln!(out, "(no sessions found)");
        return ExitCode::SUCCESS;
    }

    for m in metas {
        let mark = if m.cwd_exists { "✓" } else { "✗" };
        let date = util::format_local_short(m.last_activity);
        let base = m
            .cwd
            .as_deref()
            .and_then(|c| std::path::Path::new(c).file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "?".into());
        let title = util::truncate_to_width(&m.title, 80);
        let _ = writeln!(out, "{mark}  {date}  {base}  {title}");
    }
    ExitCode::SUCCESS
}
