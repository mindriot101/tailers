use std::fs::{File, Metadata};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use structopt::StructOpt;
use tracing::{debug, info, span, warn, Level};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct LogMessage {
    raw_line: String,
}

struct LogFileTailer<'a> {
    sender: &'a mpsc::Sender<LogMessage>,
    reader: BufReader<File>,
    // Used for logging
    span: tracing::Span,
    metadata: Metadata,
}

impl<'a> LogFileTailer<'a> {
    fn new<P>(path: P, sender: &'a mpsc::Sender<LogMessage>) -> Result<Self>
    where
        P: AsRef<Path> + std::fmt::Debug,
    {
        let f = File::open(&path)?;
        let metadata = f.metadata()?;
        let reader = BufReader::new(f);
        let span = span!(Level::INFO, "LogFileTailer", ?path);
        Ok(Self {
            sender,
            metadata,
            reader,
            span,
        })
    }

    fn run(&mut self) -> Result<thread::JoinHandle<()>> {
        Ok(thread::spawn(move || {
            self.start().unwrap();
        }))
    }

    fn start(&mut self) -> Result<()> {
        let mut buf = String::new();
        // Set up logging span
        let _entry = self.span.enter();

        loop {
            // Get the file metadata to get size
            let new_metadata = self.reader.get_ref().metadata()?;
            if new_metadata.len() <= self.metadata.len() {
                debug!("file not changed, skipping");
                continue;
            }

            // Block on reading a line
            let n = self.reader.read_line(&mut buf)?;
            debug!("got line");

            if n == 0 {
                continue;
            }

            info!("got new line");

            // Construct the log message
            let msg = LogMessage {
                raw_line: buf.clone(),
            };

            // Send the line to the receiving channel
            debug!("sending message");
            self.sender.send(msg)?;

            self.metadata = new_metadata;
            buf.clear();
        }

        Ok(())
    }
}

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

    for fname in opts.files {
        info!(%fname, "tailing log file");
        let mut tailer = LogFileTailer::new(fname, &tx)?;
        let _handle = tailer.run().expect("starting tailing thread");
    }
    drop(enter);

    info!("waiting for messages");

    for msg in rx.iter() {
        println!("{:?}", msg);
    }

    Ok(())
}
