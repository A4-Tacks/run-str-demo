use std::{env::args, process::exit};
use fs_err as fs;

use getopts_macro::getopts_options;
use run_str_demo::{Config, Rt};

struct Cfg;
impl Config for Cfg {
    fn print(&mut self, args: std::fmt::Arguments<'_>) {
        print!("{args}");
    }
}

fn main() {
    let options = getopts_options! {
        -h, --help          "show help messages";
        -v, --version       "show version messages";
    };
    let matched = match options.parse(args().skip(1)) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            exit(2)
        },
    };
    if matched.opt_present("help") {
        let usage = options.short_usage(env!("CARGO_BIN_NAME"));
        let brief = format!("{usage} <prog>");
        let help = options.usage(&brief);
        print!("{help}");
        return;
    }
    if matched.opt_present("version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Some(first) = matched.free.get(1) {
        eprintln!("Extra argument: {first:?}");
        exit(2)
    }
    let prog = matched.free.first().unwrap_or_else(|| {
        eprintln!("Expected <prog> position argument");
        exit(2)
    });
    let prog = fs::read_to_string(prog).unwrap_or_else(|e| {
        eprintln!("{e}");
        exit(1)
    });

    let mut rt = Rt::with_config(Cfg);
    rt.load_source(&prog);
    rt.proc();
}
