use std::ops::ControlFlow;

mod auto_mode;
mod batch;
mod cli;
mod commands;
mod handler;
mod logging;
mod shell;
mod utils;

use cli::PauseMode;

fn main() -> eyre::Result<()> {
    logging::init();

    if let ControlFlow::Break(result) = auto_mode::try_handle() {
        return cli::finish(result, PauseMode::OnError);
    }

    let args = cli::parse();
    let pause = args.pause;
    cli::finish(cli::run(args.command), pause)
}
