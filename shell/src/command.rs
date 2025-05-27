use tokenizer::{Token, TokenType};



pub struct CommandParser {
    pub tokens: Vec<Token>
}

impl CommandParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens }
    }

    fn append_args(&mut self, cmd: &mut Box<dyn Command>) {
        for token in self.tokens.iter().skip(1) {
            match token.kind {
                TokenType::Arg => cmd.get_args_mut().push(token.lexeme.clone()),
                _ => {}
            }
        }
    }

    fn append_flags(&mut self, cmd: &mut Box<dyn Command>) {
        for token in self.tokens.iter().skip(1) {
            match token.kind {
                TokenType::Flag => cmd.get_flags_mut().push(Flag { 
                    ident: FlagIdent::new(Some(token.lexeme.clone()), None), 
                    value: None 
                }),
                TokenType::LongFlag => cmd.get_flags_mut().push(Flag { 
                    ident: FlagIdent::new(None, Some(token.lexeme.clone())), 
                    value: None 
                }),
                TokenType::LongFlagWithValue => {
                    let parts: Vec<&str> = token.lexeme.splitn(2, '=').collect();
                    cmd.get_flags_mut().push(Flag { 
                        ident: FlagIdent::new(None, Some(parts[0].to_string())), 
                        value: Some(parts[1].to_string()) 
                    })
                },
                _ => {}
            }
        }
    }

    pub fn parse(&mut self) -> Result<Box<dyn Command>, String> {
        let mut cmd: Box<dyn Command> = match self.tokens[0].lexeme.as_str() {
            "cd" => Box::new(ChangeDirCommand::new()),
            _ => return Err(format!("Unknown command: {}", self.tokens[0].lexeme)),
        };

        self.append_args(&mut cmd);
        self.append_flags(&mut cmd);

        Ok(cmd)
    }
}

#[derive(Default)]
pub struct IoRedirection {
    pub from: Option<Box<dyn std::io::Read>>,
    pub to: Option<Box<dyn std::io::Write>>,
    pub error: Option<Box<dyn std::io::Write>>,
}

pub trait Command {
    fn get_name(&self) -> &str;
    fn get_args(&self) -> &[String];
    fn get_flags(&self) -> &[Flag];
    fn get_io_redirection(&mut self) -> &mut IoRedirection;
    fn set_output(&mut self, output: Box<dyn std::io::Write>) {
        self.get_io_redirection().to = Some(output);
    }
    fn set_error(&mut self, error: Box<dyn std::io::Write>) {
        self.get_io_redirection().error = Some(error);
    }
    fn set_input(&mut self, input: Box<dyn std::io::Read>) {
        self.get_io_redirection().from = Some(input);
    }
    fn get_args_mut(&mut self) -> &mut Vec<String>;
    fn get_flags_mut(&mut self) -> &mut Vec<Flag>;
    fn execute(&self) -> Result<(), Box<dyn std::error::Error>>;
}


#[derive(Clone, Debug)]
pub struct FlagIdent {
    pub short: Option<String>, 
    pub long: Option<String>,
}

impl FlagIdent {
    pub fn new(short: Option<String>, long: Option<String>) -> Self {
        Self { short, long }
    }
}

#[derive(Clone, Debug)]
pub struct Flag {
    pub ident: FlagIdent, 
    pub value: Option<String>,
}

pub struct ChangeDirCommand {
    pub name: String,
    pub args: Vec<String>,
    pub flags: Vec<Flag>,
    pub io_redirection: IoRedirection,
}

impl ChangeDirCommand {
    pub fn new() -> Self {
        Self { name: "cd".to_string(), args: vec![], flags: vec![], io_redirection: IoRedirection::default() }
    }
}

impl Command for ChangeDirCommand {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_args(&self) -> &[String] {
        &self.args
    }

    fn get_flags(&self) -> &[Flag] {
        &self.flags
    }

    fn get_args_mut(&mut self) -> &mut Vec<String> {
        &mut self.args
    }

    fn get_flags_mut(&mut self) -> &mut Vec<Flag> {
        &mut self.flags
    }

    fn execute(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.args.get(0).ok_or("No path provided")?;
        std::env::set_current_dir(path)?;
        Ok(())
    }
    
    fn get_io_redirection(&mut self) -> &mut IoRedirection {
        &mut self.io_redirection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenizer::Tokenizer;

    fn create_tokens(input: &str) -> Vec<Token> {
        let mut tokenizer = Tokenizer::new(input.to_string());
        tokenizer.scan_tokens();
        tokenizer.tokens
    }

    #[test]
    fn test_io_redirection() {
        let mut cmd = ChangeDirCommand::new();
        cmd.set_output(Box::new(std::io::Cursor::new(Vec::new())));
        cmd.set_error(Box::new(std::io::Cursor::new(Vec::new())));
        cmd.set_input(Box::new(std::io::Cursor::new(Vec::new())));
        assert!(cmd.get_io_redirection().to.is_some());
        assert!(cmd.get_io_redirection().error.is_some());
        assert!(cmd.get_io_redirection().from.is_some());
    }

    #[test]
    fn test_parse_cd_command() {
        let tokens = create_tokens("cd /tmp");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();

        assert_eq!(cmd.get_name(), "cd");
        assert_eq!(cmd.get_args(), &["/tmp"]);
        assert!(cmd.get_flags().is_empty());
    }

    #[test]
    fn test_parse_cd_with_flags() {
        let tokens = create_tokens("cd --follow-symlinks /tmp");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();

        assert_eq!(cmd.get_name(), "cd");
        assert_eq!(cmd.get_args(), &["/tmp"]);
        
        let flags = cmd.get_flags();
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].ident.long, Some("--follow-symlinks".to_string()));
        assert_eq!(flags[0].ident.short, None);
        assert_eq!(flags[0].value, None);
    }

    #[test]
    fn test_parse_cd_with_value_flag() {
        let tokens = create_tokens("cd --format=list /tmp");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();

        assert_eq!(cmd.get_name(), "cd");
        assert_eq!(cmd.get_args(), &["/tmp"]);
        
        let flags = cmd.get_flags();
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].ident.long, Some("--format".to_string()));
        assert_eq!(flags[0].value, Some("list".to_string()));
    }

    #[test]
    fn test_parse_cd_with_short_flag() {
        let tokens = create_tokens("cd -l /tmp");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();

        assert_eq!(cmd.get_name(), "cd");
        assert_eq!(cmd.get_args(), &["/tmp"]);
        
        let flags = cmd.get_flags();
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].ident.short, Some("-l".to_string()));
        assert_eq!(flags[0].ident.long, None);
        assert_eq!(flags[0].value, None);
    }

    #[test]
    fn test_parse_unknown_command() {
        let tokens = create_tokens("unknown_cmd arg1 arg2");
        let mut parser = CommandParser::new(tokens);
        
        match parser.parse() {
            Ok(_) => panic!("Expected error for unknown command"),
            Err(e) => assert_eq!(e, "Unknown command: unknown_cmd"),
        }
    }

    #[test]
    fn test_cd_command_execution() {
        use std::env;
        use std::path::PathBuf;
        
        // Save current directory
        let original_dir = env::current_dir().unwrap();
        
        // Create tokens and execute cd command
        let tokens = create_tokens("cd /tmp");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        let result = cmd.execute();
        
        // Verify command execution
        assert!(result.is_ok());
        
        let current_dir = env::current_dir().unwrap();
        let tmp_path = PathBuf::from("/tmp").canonicalize().unwrap();
        assert_eq!(current_dir, tmp_path);
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_cd_command_execution_error() {
        let tokens = create_tokens("cd /nonexistent/directory");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        let result = cmd.execute();
        
        assert!(result.is_err());
    }

    #[test]
    fn test_cd_command_no_path() {
        let tokens = create_tokens("cd");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        match cmd.execute() {
            Ok(_) => panic!("Expected error for missing path"),
            Err(e) => assert_eq!(e.to_string(), "No path provided"),
        }
    }

    #[test]
    fn test_flag_ident_creation() {
        let flag = FlagIdent::new(Some("-l".to_string()), None);
        assert_eq!(flag.short, Some("-l".to_string()));
        assert_eq!(flag.long, None);

        let flag = FlagIdent::new(None, Some("--long".to_string()));
        assert_eq!(flag.short, None);
        assert_eq!(flag.long, Some("--long".to_string()));
    }

    #[test]
    fn test_command_creation() {
        let cmd = ChangeDirCommand::new();
        assert_eq!(cmd.get_name(), "cd");
        assert!(cmd.get_args().is_empty());
        assert!(cmd.get_flags().is_empty());
    }
}

