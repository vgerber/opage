use clap::{Arg, Command};

pub fn cli() -> Command {
    Command::new("opage")
        .about("OpenAPI v3.1 client generator")
        .arg(
            Arg::new("output-dir")
                .short('o')
                .help("Client output location")
                .required(true),
        )
        .arg(
            Arg::new("spec")
                .short('s')
                .help("Input OpenAPI spec")
                .required(true),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .help("(json) Configuration with name mappings and ignores")
                .required(false),
        )
}
