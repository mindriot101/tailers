use chrono::prelude::*;
use notify::{op::Op, raw_watcher, RawEvent, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use structopt::StructOpt;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use tracing::{event, span, Level};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, StructOpt)]
#[structopt(name = "tailers", version = "0.1.0")]
struct Opt {
    #[structopt(short, long)]
    files: Vec<String>,
    // TODO: docker
    // TODO: kubernetes
}

#[derive(Debug)]
struct LogEvent {
    filename: PathBuf,
    line: String,
    tailer_idx: usize,
}

struct LogFileTailer {
    // Sender for the outer channel which broadcasts log messages
    sender: Arc<Mutex<mpsc::Sender<LogEvent>>>,

    // Receiver for the notifications
    rx: Arc<Mutex<mpsc::Receiver<RawEvent>>>,
    // TODO: make this generic over this parameter to support other operating systems
    _watcher: notify::inotify::INotifyWatcher,

    reader: Arc<Mutex<BufReader<File>>>,

    tailer_idx: usize,
}

impl LogFileTailer {
    fn new<P>(path: P, tailer_idx: usize, sender: mpsc::Sender<LogEvent>) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let (tx, rx) = mpsc::channel();
        // TODO: single shared watcher for the whole crate?
        let mut watcher = raw_watcher(tx)?;
        watcher.watch(&path, RecursiveMode::NonRecursive)?;

        // Get the last seek position of the file to continue from
        let mut f = File::open(&path)?;
        f.seek(SeekFrom::End(0))?;
        let reader = BufReader::new(f);

        Ok(Self {
            sender: Arc::new(Mutex::new(sender)),
            rx: Arc::new(Mutex::new(rx)),
            _watcher: watcher,
            reader: Arc::new(Mutex::new(reader)),
            tailer_idx,
        })
    }

    fn start(&mut self) {
        let rx = self.rx.clone();
        let sender = self.sender.clone();
        let reader = self.reader.clone();
        let mut buf = String::new();

        let span = span!(Level::DEBUG, "start");
        let tailer_idx = self.tailer_idx;

        thread::spawn(move || {
            let _enter = span.enter();
            loop {
                let rx = rx.lock().unwrap();
                match rx.recv() {
                    Ok(RawEvent {
                        path: Some(path),
                        op: Ok(Op::WRITE),
                        cookie: _cookie,
                    }) => {
                        event!(Level::DEBUG, ?path, "file write notification");
                        let sender = sender.lock().unwrap();
                        let mut reader = reader.lock().unwrap();

                        loop {
                            match reader.read_line(&mut buf) {
                                // we have reached the end of the file
                                Ok(0) => break,
                                Ok(_) => {
                                    event!(Level::DEBUG, "sending line");
                                    sender
                                        .send(LogEvent {
                                            filename: path.clone(),
                                            line: buf.clone(),
                                            tailer_idx,
                                        })
                                        .unwrap()
                                }
                                Err(e) => return Err(e),
                            }

                            buf.clear();
                        }
                    }
                    Ok(e) => {
                        // Some other event
                        // TODO: handle file renaming or deletion
                        event!(Level::WARN, ?e, "unhandled file event");
                    }
                    Err(err) => {
                        eprintln!("error: {:?}", err);
                        break Ok(());
                    }
                }
            }
        });
    }
}

trait Tailer {}

impl Tailer for LogFileTailer {}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    event!(Level::INFO, "Program starting");

    let opts = Opt::from_args();

    // Message channel for receiving new messages from all tailers
    let (tx, rx) = mpsc::channel();

    // Keep track of all tailers so that their channels do not get closed
    let mut tailers: Vec<Box<dyn Tailer>> = Vec::new();
    let mut tailer_idx = 0;
    let mut tailer_colours = HashMap::new();

    let span = span!(Level::INFO, "adding-files");
    let enter = span.enter();

    // Start by adding the files requested
    for file in opts.files {
        event!(Level::INFO, ?file);
        let mut tailer = LogFileTailer::new(file, tailer_idx, tx.clone())?;
        tailer.start();
        tailers.push(Box::new(tailer));

        let rcolour = random_color::RandomColor::new().to_rgb_array();

        let colour = Color::Rgb(rcolour[0] as _, rcolour[1] as _, rcolour[2] as _);

        tailer_colours.insert(tailer_idx, colour);
        tailer_idx += 1;
    }

    // Clear the span
    drop(enter);

    event!(Level::INFO, "watching {} sources", tailers.len());

    let mut stdout = StandardStream::stdout(ColorChoice::Auto);

    loop {
        let event = rx.recv()?;
        if let Some(p) = event.filename.to_str() {
            let utc: DateTime<Utc> = Utc::now();

            write!(stdout, "{} [", utc)?;
            stdout.set_color(ColorSpec::new().set_fg(Some(tailer_colours[&event.tailer_idx])))?;
            write!(stdout, "{}", p)?;
            stdout.reset()?;
            writeln!(stdout, "]: {}", event.line.trim())?;
        }
    }
}
