use cli::{CliCommand, parse_args, print_help, print_version};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse_args(&args) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(err.exit_code());
        }
    };

    match command {
        CliCommand::Help => {
            print_help();
        }
        CliCommand::Version => {
            print_version();
        }
        CliCommand::LanguageServer => {
            lsp::run_stdio_server().await;
        }
        CliCommand::Update(sub_args) => {
            if let Err(err) = update::run(&sub_args) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        CliCommand::Lint(sub_args) => {
            if let Err(err) = lint::run(&sub_args) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
        CliCommand::Format(sub_args) => {
            if let Err(err) = format::run(&sub_args) {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    }
}
