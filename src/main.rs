use clap::Parser;
use envault::cli::{run_cli, Cli};

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    if args.command.is_some() {
        run_cli(args)?;
    } else {
        println!("envault: run with --help to see available commands or specify a subcommand");
    }
    Ok(())
}
