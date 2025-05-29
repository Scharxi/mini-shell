#[derive(Debug, PartialEq)]
pub enum TokenType {
    Cmd,
    Arg,
    Flag,
    LongFlag,
    LongFlagWithValue,
    Pipe,           // |
    InputRedir,     // <
    OutputRedir,    // >
    Background,     // &
    Eof,
}

#[derive(Debug)]
pub struct Token {
    pub kind: TokenType,
    pub lexeme: String,
}

pub struct Tokenizer {
    pub tokens: Vec<Token>,
    pub source: String,
    pub start: usize,
    pub current: usize,
    had_cmd: bool,
}

impl Tokenizer {
    pub fn new(source: String) -> Self {
        Self {
            tokens: Vec::new(),
            source,
            start: 0,
            current: 0,
            had_cmd: false,
        }
    }

    pub fn scan_tokens(&mut self) {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }
        self.tokens.push(Token {
            kind: TokenType::Eof,
            lexeme: "".to_string(),
        });
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn scan_token(&mut self) {
        let c = self.advance();
        match c {
            ' ' | '\r' | '\t' | '\n' => self.skip_whitespace(),
            '-' => {
                if self.match_char('-') {
                    self.handle_long_flag();
                } else {
                    self.handle_flag();
                }
            }
            '|' => {
                self.add_token(TokenType::Pipe);
                self.had_cmd = false; // Reset had_cmd after pipe to allow new command
            },
            '<' => self.add_token(TokenType::InputRedir),
            '>' => self.add_token(TokenType::OutputRedir),
            '&' => self.add_token(TokenType::Background),
            _ => self.handle_word(),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\r' | '\t' | '\n' => { self.advance(); }
                _ => break,
            }
        }
    }

    fn handle_long_flag(&mut self) {
        let mut has_equals = false;
        
        while let Some(c) = self.peek() {
            if c == '=' {
                has_equals = true;
                self.advance();
                self.handle_flag_value();
                break;
            } else if c.is_ascii_alphanumeric() || c == '-' {
                self.advance();
            } else {
                break;
            }
        }

        if !has_equals {
            self.add_token(TokenType::LongFlag);
        }
    }

    fn handle_flag_value(&mut self) {
        let value_start = self.current;
        let mut in_quotes = false;

        while let Some(c) = self.peek() {
            match c {
                '"' => {
                    in_quotes = !in_quotes;
                    self.advance();
                }
                ' ' if !in_quotes => break,
                _ => {
                    self.advance();
                }
            }
        }

        // Combine flag and value into a single token
        let flag = self.source[self.start..value_start-1].trim().to_string(); // -1 to exclude the equals sign
        let value = self.source[value_start..self.current].trim_matches('"').to_string();
        self.tokens.push(Token {
            kind: TokenType::LongFlagWithValue,
            lexeme: format!("{}={}", flag, value),
        });
    }

    fn handle_flag(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() {
                self.advance();
            } else {
                break;
            }
        }
        self.add_token(TokenType::Flag);
    }

    fn handle_word(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' || c == '/' || c == '.' {
                self.advance();
            } else {
                break;
            }
        }
        
        if !self.had_cmd {
            self.add_token(TokenType::Cmd);
            self.had_cmd = true;
        } else {
            self.add_token(TokenType::Arg);
        }
    }

    fn peek(&self) -> Option<char> {
        if self.is_at_end() {
            None
        } else {
            Some(self.source.chars().nth(self.current).unwrap())
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if let Some(c) = self.peek() {
            if c == expected {
                self.advance();
                return true;
            }
        }
        false
    }

    fn advance(&mut self) -> char {
        let c = self.source.chars().nth(self.current).unwrap();
        self.current += 1;
        c
    }

    fn add_token(&mut self, kind: TokenType) {
        let text = self.source[self.start..self.current].trim().to_string();
        if !text.is_empty() {
            self.tokens.push(Token { kind, lexeme: text });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_command() {
        let mut tokenizer = Tokenizer::new("ls -l".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 3);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "ls");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::Flag);
        assert_eq!(tokenizer.tokens[1].lexeme, "-l");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Eof);
    }

    #[test]
    fn test_command_with_args() {
        let mut tokenizer = Tokenizer::new("cp file1.txt file2.txt".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 4);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "cp");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[1].lexeme, "file1.txt");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[2].lexeme, "file2.txt");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Eof);
    }

    #[test]
    fn test_command_with_flags_and_args() {
        let mut tokenizer = Tokenizer::new("grep -r pattern /path/to/dir".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 5);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "grep");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::Flag);
        assert_eq!(tokenizer.tokens[1].lexeme, "-r");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[2].lexeme, "pattern");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[3].lexeme, "/path/to/dir");
        assert_eq!(tokenizer.tokens[4].kind, TokenType::Eof);
    }

    #[test]
    fn test_long_flags() {
        let mut tokenizer = Tokenizer::new("cp --hello-world --format=json file.txt".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 5);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "cp");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::LongFlag);
        assert_eq!(tokenizer.tokens[1].lexeme, "--hello-world");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::LongFlagWithValue);
        assert_eq!(tokenizer.tokens[2].lexeme, "--format=json");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[3].lexeme, "file.txt");
        assert_eq!(tokenizer.tokens[4].kind, TokenType::Eof);
    }

    #[test]
    fn test_long_flags_with_quoted_values() {
        let mut tokenizer = Tokenizer::new(r#"cp --format="json with spaces" file.txt"#.to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 4);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "cp");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::LongFlagWithValue);
        assert_eq!(tokenizer.tokens[1].lexeme, "--format=json with spaces");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[2].lexeme, "file.txt");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Eof);
    }

    #[test]
    fn test_pipe_operator() {
        let mut tokenizer = Tokenizer::new("ls -l | grep pattern".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 6);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "ls");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::Flag);
        assert_eq!(tokenizer.tokens[1].lexeme, "-l");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Pipe);
        assert_eq!(tokenizer.tokens[2].lexeme, "|");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[3].lexeme, "grep");
        assert_eq!(tokenizer.tokens[4].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[4].lexeme, "pattern");
        assert_eq!(tokenizer.tokens[5].kind, TokenType::Eof);
    }

    #[test]
    fn test_redirection_operators() {
        let mut tokenizer = Tokenizer::new("cat < input.txt > output.txt".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 6);  // 5 tokens + EOF
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "cat");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::InputRedir);
        assert_eq!(tokenizer.tokens[1].lexeme, "<");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[2].lexeme, "input.txt");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::OutputRedir);
        assert_eq!(tokenizer.tokens[3].lexeme, ">");
        assert_eq!(tokenizer.tokens[4].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[4].lexeme, "output.txt");
        assert_eq!(tokenizer.tokens[5].kind, TokenType::Eof);
    }

    #[test]
    fn test_background_operator() {
        let mut tokenizer = Tokenizer::new("sleep 10 &".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 4);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "sleep");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[1].lexeme, "10");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Background);
        assert_eq!(tokenizer.tokens[2].lexeme, "&");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Eof);
    }

    #[test]
    fn test_complex_command() {
        let mut tokenizer = Tokenizer::new("cat file.txt | grep pattern > output.txt &".to_string());
        tokenizer.scan_tokens();

        assert_eq!(tokenizer.tokens.len(), 9);
        assert_eq!(tokenizer.tokens[0].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[0].lexeme, "cat");
        assert_eq!(tokenizer.tokens[1].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[1].lexeme, "file.txt");
        assert_eq!(tokenizer.tokens[2].kind, TokenType::Pipe);
        assert_eq!(tokenizer.tokens[2].lexeme, "|");
        assert_eq!(tokenizer.tokens[3].kind, TokenType::Cmd);
        assert_eq!(tokenizer.tokens[3].lexeme, "grep");
        assert_eq!(tokenizer.tokens[4].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[4].lexeme, "pattern");
        assert_eq!(tokenizer.tokens[5].kind, TokenType::OutputRedir);
        assert_eq!(tokenizer.tokens[5].lexeme, ">");
        assert_eq!(tokenizer.tokens[6].kind, TokenType::Arg);
        assert_eq!(tokenizer.tokens[6].lexeme, "output.txt");
        assert_eq!(tokenizer.tokens[7].kind, TokenType::Background);
        assert_eq!(tokenizer.tokens[7].lexeme, "&");
        assert_eq!(tokenizer.tokens[8].kind, TokenType::Eof);
    }
}