pub mod command;

use std::{fs::File, io::{BufRead, BufReader, ErrorKind, Write}, path::PathBuf, process, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};
use command::CommandParser;
use tokenizer::Tokenizer;

static RUNNING: AtomicBool = AtomicBool::new(true);

pub struct History {
    pub commands: Vec<String>,
}

impl Iterator for History {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        self.commands.pop()
    }
}

impl History {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    pub fn append<T: ToString>(&mut self, command: T) {
        self.commands.push(command.to_string());
    }

    fn get_history_file_path() -> PathBuf {
        let mut path = if let Some(home) = dirs::home_dir() {
            home
        } else {
            PathBuf::from(".")
        };
        path.push(".msh_history");
        path
    }

    pub fn load_from_disk() -> Result<Self, Box<dyn std::error::Error>> {
        let history_path = Self::get_history_file_path();
        let mut history = Self::new();
        if let Ok(file) = File::open(&history_path) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                history.commands.push(line?);
            }
        } else {
            println!("No history file found at {}", history_path.display());
        }

        Ok(history)
    }

    pub fn save(&self) {
        let history_path = Self::get_history_file_path();
        if let Ok(mut file) = File::create(&history_path) {
            for command in self.commands.iter() {
                let _ = writeln!(file, "{}", command);
            }
            let _ = file.flush();
            println!("History saved to {}", history_path.display());
        } else {
            eprintln!("Failed to save history to {}", history_path.display());
        }
    }
}

pub struct Shell {
    pub base_path: String, 
    pub history: Arc<Mutex<History>>,
}

impl Shell {
    pub fn new() -> Self {
        let history = Arc::new(Mutex::new(History::new()));
        let history_clone = Arc::clone(&history);

        ctrlc::set_handler(move || {
            println!("\nReceived Ctrl+C! Saving history and exiting...");
            if let Ok(history) = history_clone.lock() {
                history.save();
            }
            process::exit(0);
        }).expect("Error setting Ctrl+C handler");

        Self { 
            base_path: std::env::current_dir()
                .unwrap_or_default()
                .to_str()
                .unwrap_or(".")
                .to_string(), 
            history 
        }
    }

    pub fn put_line(&self, msg: &str) {
        print!("{}", msg);
        let _ = std::io::stdout().flush();
    } 

    pub fn put_prefixed_line(&self, msg: &str) {
        print!("shell> {}", msg);
        let _ = std::io::stdout().flush();
    }

    pub fn eval(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("History will be saved to {}", History::get_history_file_path().display());
        self.put_prefixed_line("");
        while RUNNING.load(Ordering::SeqCst) {
            match self.read_line() {
                Some(line) => {
                    let trimmed = line.trim();
                    if trimmed == "exit" || trimmed == "quit" {
                        println!("\nGoodbye!");
                        if let Ok(history) = self.history.lock() {
                            history.save();
                        }
                        return Ok(());
                    }

                    let mut tokenizer = Tokenizer::new(line);
                    tokenizer.scan_tokens();
                    
                    let mut parser = CommandParser::new(tokenizer.tokens);
                    match parser.parse() {
                        Ok(cmd) => {
                            if let Err(e) = cmd.execute() {
                                println!("Error: {}", e);
                            }
                            if let Ok(mut history) = self.history.lock() {
                                history.append(cmd.to_string());
                            }
                        }
                        Err(e) => println!("Error: {}", e),
                    }
                    
                    self.put_prefixed_line("");
                }
                None => {
                    self.put_prefixed_line("");
                }
            }
        }
        Ok(())
    }

    fn read_line(&self) -> Option<String> { 
        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(_) => Some(line),
            Err(e) => {
                if e.kind() == ErrorKind::Interrupted {
                    None
                } else {
                    Some(line)
                }
            }
        }
    }
}