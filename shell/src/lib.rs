use std::io::Write;
use tokenizer::{Tokenizer, Token};

pub struct Shell {
}

impl Shell {
    pub fn new() -> Self {
        Self {}
    }

    pub fn put_line(&self, msg: &str) {
        print!("{}", msg);
        std::io::stdout().flush().unwrap();
    } 

    pub fn put_prefixed_line(&self, msg: &str) {
        print!("shell> {}", msg);
        std::io::stdout().flush().unwrap();
    }

    pub fn eval(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.put_prefixed_line("");
        loop {
            if let Some(line) = self.read_line() {
                let mut tokenizer = Tokenizer::new(line);
                tokenizer.scan_tokens();
                
                for token in tokenizer.tokens {
                    match token.kind {
                        tokenizer::TokenType::Cmd => self.execute_command(&token),
                        _ => continue,
                    }
                }
                
                self.put_prefixed_line("");
            } else {
                self.put_prefixed_line("");
            }
        }
    }

    fn execute_command(&self, cmd: &Token) {
        self.put_line(&format!("Executing command: {}\n", cmd.lexeme));
    }

    fn read_line(&self) -> Option<String> { 
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok()?;
        Some(line)
    }
}