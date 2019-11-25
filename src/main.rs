use notify::{raw_watcher, RawEvent, RecursiveMode, Watcher};
use std::sync::mpsc;
use structopt::StructOpt;
use tracing::{debug, info, span, warn, Level};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, StructOpt)]
#[structopt(name = "tailers", version = "0.1.0")]
struct Opt {
    #[structopt(short, long)]
    files: Vec<String>,
    // TODO: docker
    // TODO: kubernetes
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Program starting");

    let opts = Opt::from_args();

    // Check that we have actually been given some files to log
    // TODO: check the other arguments as well
    if opts.files.len() == 0 {
        warn!("no tailers requested, quitting");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel();

    let span = span!(Level::TRACE, "adding-files");
    let enter = span.enter();

    let mut watcher = raw_watcher(tx)?;
    for fname in opts.files {
        info!(%fname, "tailing log file");
        watcher.watch(fname, RecursiveMode::NonRecursive)?;
    }
    drop(enter);

    info!("waiting for messages");
    loop {
        match rx.recv() {
            Ok(RawEvent {
                path: Some(path),
                op: Ok(op),
                cookie: _cookie,
            }) => {
                println!("got raw event {:?}, {:?}", path, op);
            }
            Ok(event) => {
                eprintln!("got broken event: {:?}", event);
                break;
            }
            Err(err) => {
                eprintln!("error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
