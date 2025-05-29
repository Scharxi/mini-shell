use tokenizer::{Token, TokenType};
use std::io::Write;

use crate::History;



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

    fn parse_single_command(&mut self, start: usize, end: usize) -> Result<Box<dyn Command>, String> {
        let cmd_token = &self.tokens[start];
        if cmd_token.kind != TokenType::Cmd {
            return Err(format!("Expected command, got: {}", cmd_token.lexeme));
        }

        let cmd: Box<dyn Command> = match cmd_token.lexeme.as_str() {
            "cd" => Box::new(ChangeDirCommand::new()),
            "history" => Box::new(HistoryCommand::new()),
            "pwd" => Box::new(PwdCommand::new()),
            _ => Box::new(SystemCommand::new(cmd_token.lexeme.clone())),
        };

        let mut cmd = cmd;
        
        // Parse args and flags for this command segment
        for token in self.tokens[start + 1..end].iter() {
            match token.kind {
                TokenType::Arg => cmd.get_args_mut().push(token.lexeme.clone()),
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

        Ok(cmd)
    }

    pub fn parse(&mut self) -> Result<Box<dyn Command>, String> {
        if self.tokens.is_empty() {
            return Err("No command provided".to_string());
        }

        // Check if we have any pipes
        let pipe_positions: Vec<usize> = self.tokens.iter()
            .enumerate()
            .filter(|(_, token)| token.kind == TokenType::Pipe)
            .map(|(i, _)| i)
            .collect();

        // If no pipes, parse as a single command
        if pipe_positions.is_empty() {
            return self.parse_single_command(0, self.tokens.len());
        }

        // Create a pipeline for multiple commands
        let mut pipeline = Pipeline::new();
        let mut start = 0;

        // Parse each command in the pipeline
        for &pipe_pos in &pipe_positions {
            let cmd = self.parse_single_command(start, pipe_pos)?;
            pipeline.add_command(cmd);
            start = pipe_pos + 1;
        }

        // Parse the last command after the last pipe
        let last_cmd = self.parse_single_command(start, self.tokens.len())?;
        pipeline.add_command(last_cmd);

        Ok(Box::new(pipeline))
    }
}

// Make Pipeline implement Command trait
impl Command for Pipeline {
    fn get_name(&self) -> &str {
        "pipeline"
    }

    fn get_args(&self) -> &[String] {
        &[]
    }

    fn get_flags(&self) -> &[Flag] {
        &[]
    }

    fn get_args_mut(&mut self) -> &mut Vec<String> {
        unimplemented!("Pipeline does not support mutable arguments")
    }

    fn get_flags_mut(&mut self) -> &mut Vec<Flag> {
        unimplemented!("Pipeline does not support mutable flags")
    }

    fn get_io_redirection(&mut self) -> &mut IoRedirection {
        unimplemented!("Pipeline handles I/O internally")
    }

    fn execute_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.commands.is_empty() {
            return Ok(());
        }

        // Single command case - no pipes needed
        if self.commands.len() == 1 {
            return self.commands[0].execute();
        }

        // Multiple commands case - need to set up pipes
        use std::process::{Command as ProcessCommand, Stdio};
        
        let mut handles = Vec::new();
        
        // Create all the necessary pipes
        for i in 0..self.commands.len() - 1 {
            let cmd = &self.commands[i];
            let system_cmd = match cmd.get_name() {
                "cd" | "pwd" | "history" => {
                    return Err("Built-in commands cannot be used in pipes".into());
                }
                name => name,
            };

            let mut process = ProcessCommand::new(system_cmd);
            process.args(cmd.get_args());
            process.stdout(Stdio::piped());
            
            if i > 0 {
                process.stdin(Stdio::piped());
            }
            
            handles.push(process);
        }

        // Handle the last command separately
        let last_cmd = &self.commands[self.commands.len() - 1];
        let system_cmd = match last_cmd.get_name() {
            "cd" | "pwd" | "history" => {
                return Err("Built-in commands cannot be used in pipes".into());
            }
            name => name,
        };
        
        let mut last_process = ProcessCommand::new(system_cmd);
        last_process.args(last_cmd.get_args());
        last_process.stdin(Stdio::piped());
        handles.push(last_process);

        // Execute the pipeline
        let mut previous_child: Option<std::process::Child> = None;
        
        for (i, mut process) in handles.into_iter().enumerate() {
            if let Some(prev) = previous_child {
                process.stdin(Stdio::from(prev.stdout.unwrap()));
            }
            
            let child = process.spawn()?;
            previous_child = Some(child);
        }

        // Wait for the last process to complete
        if let Some(mut last_child) = previous_child {
            let status = last_child.wait()?;
            if !status.success() {
                return Err(format!("Pipeline failed with status: {}", status).into());
            }
        }

        Ok(())
    }

    fn get_help(&self) -> CommandHelp {
        CommandHelp {
            short_desc: "A pipeline of commands".to_string(),
            long_desc: "Executes multiple commands in sequence, connecting their standard output to standard input.".to_string(),
            usage: "command1 | command2 [| command3 ...]".to_string(),
            flags: vec![],
        }
    }
}

#[derive(Default)]
pub struct IoRedirection {
    pub from: Option<Box<dyn std::io::Read>>,
    pub to: Option<Box<dyn std::io::Write>>,
    pub error: Option<Box<dyn std::io::Write>>,
}

pub struct CommandHelp {
    pub short_desc: String,
    pub long_desc: String,
    pub usage: String,
    pub flags: Vec<(String, String)>, // (flag, description)
}

pub trait Command {
    fn get_name(&self) -> &str;
    fn get_args(&self) -> &[String];
    fn get_args_len(&self) -> usize {
        self.get_args().len()
    }
    fn get_flags(&self) -> &[Flag];
    fn get_flag(&self, flag: &str) -> Option<&Flag> {
        let flag_ident = FlagIdent::try_from(flag.to_string()).ok()?;
        self.get_flags().iter().find(|f| f.ident == flag_ident)
    }
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
    fn get_input_mut(&mut self) -> &mut Box<dyn std::io::Read> {
        self.get_io_redirection().from.as_mut().unwrap()
    }
    fn get_output_mut(&mut self) -> &mut Box<dyn std::io::Write> {
        self.get_io_redirection().to.as_mut().unwrap()
    }
    fn get_error_mut(&mut self) -> &mut Box<dyn std::io::Write> {
        self.get_io_redirection().error.as_mut().unwrap()
    }
    fn get_args_mut(&mut self) -> &mut Vec<String>;
    fn get_flags_mut(&mut self) -> &mut Vec<Flag>;
    fn execute(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Check for help flag first
        if self.get_flag("--help").is_some() || self.get_flag("-h").is_some() {
            self.print_help();
            return Ok(());
        }
        self.execute_impl()
    }
    fn execute_impl(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn get_help(&self) -> CommandHelp;
    fn print_help(&self) {
        let help = self.get_help();
        println!("{}:", self.get_name().to_uppercase());
        println!("  {}\n", help.short_desc);
        println!("Description:");
        println!("  {}\n", help.long_desc);
        println!("Usage:");
        println!("  {}\n", help.usage);
        if !help.flags.is_empty() {
            println!("Flags:");
            for (flag, desc) in help.flags {
                println!("  {:<20} {}", flag, desc);
            }
        }
    }
}

impl ToString for dyn Command {
    fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str(self.get_name());
        result.push_str(" ");
        result.push_str(self.get_args().join(" ").as_str());
        result.push_str(self.get_flags().iter().map(|flag| flag.ident.to_string()).collect::<Vec<String>>().join(" ").as_str());
        result
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlagIdent {
    pub short: Option<String>, 
    pub long: Option<String>,
}

impl TryFrom<String> for FlagIdent {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.starts_with("--") {
            Ok(Self { short: None, long: Some(value) })
        } else if value.starts_with("-") {
            Ok(Self { short: Some(value), long: None })
        } else {
            Err(format!("Invalid flag: {}", value))
        }
    }
}

impl TryFrom<&str> for FlagIdent {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.starts_with("--") {
            Ok(Self { short: None, long: Some(value.to_string()) })
        } else if value.starts_with("-"){
            Ok(Self { short: Some(value.to_string()), long: None })
        } else {
            Err(format!("Invalid flag: {}", value))
        }
    }
}

impl ToString for FlagIdent {
    fn to_string(&self) -> String {
        let mut result = String::new();
        if let Some(short) = self.short.clone() {
            result.push_str(short.as_str());
            return result;
        }
        if let Some(long) = self.long.clone() {
            result.push_str(long.as_str());
            return result;
        }
        result
    }
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

pub struct PwdCommand {
    pub name: String,
    pub io_redirection: IoRedirection,
}

impl PwdCommand {
    pub fn new() -> Self {
        Self { name: "pwd".to_string(), io_redirection: IoRedirection::default() }
    }
}

impl Command for PwdCommand {
    fn get_name(&self) -> &str {
        &self.name
    }
    
    fn get_args(&self) -> &[String] {
        &[]
    }
    
    fn get_flags(&self) -> &[Flag] {
        &[]
    }
    
    fn get_io_redirection(&mut self) -> &mut IoRedirection {
        &mut self.io_redirection
    }
    fn get_args_mut(&mut self) -> &mut Vec<String> {
        unimplemented!("PwdCommand does not support mutable arguments")
    }
    
    fn get_flags_mut(&mut self) -> &mut Vec<Flag> {
        unimplemented!("PwdCommand does not support mutable flags") 
    }
    
    fn execute_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::current_dir()?;
        println!("{}", path.display());
        Ok(())
    }

    fn get_help(&self) -> CommandHelp {
        CommandHelp {
            short_desc: "Print the current working directory".to_string(),
            long_desc: "Display the full path of the current working directory.".to_string(),
            usage: "pwd [flags]".to_string(),
            flags: vec![
                ("--help, -h".to_string(), "Show this help message".to_string()),
            ],
        }
    }
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

    fn execute_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.args.get(0).ok_or("No path provided")?;
        std::env::set_current_dir(path)?;
        Ok(())
    }
    
    fn get_io_redirection(&mut self) -> &mut IoRedirection {
        &mut self.io_redirection
    }

    fn get_help(&self) -> CommandHelp {
        CommandHelp {
            short_desc: "Change the current working directory".to_string(),
            long_desc: "Change the shell's current working directory to the specified path. \
                       If no path is provided, an error will be displayed.".to_string(),
            usage: "cd [flags] <path>".to_string(),
            flags: vec![
                ("--help, -h".to_string(), "Show this help message".to_string()),
                ("--follow-symlinks".to_string(), "Follow symbolic links".to_string()),
            ],
        }
    }
}

pub struct HistoryCommand {
    pub name: String, 
    pub args: Vec<String>,
    pub flags: Vec<Flag>,
    pub io_redirection: IoRedirection,
}
impl HistoryCommand {
    fn new() -> Self {
        Self { name: "history".to_string(), args: vec![], flags: vec![], io_redirection: IoRedirection::default() }
    }
}

impl Command for HistoryCommand {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_args(&self) -> &[String] {
        &self.args
    }

    fn get_flags(&self) -> &[Flag] {
        &self.flags
    }

    fn get_io_redirection(&mut self) -> &mut IoRedirection {
        &mut self.io_redirection
    }

    fn get_args_mut(&mut self) -> &mut Vec<String> {
        &mut self.args
    }

    fn get_flags_mut(&mut self) -> &mut Vec<Flag> {
        &mut self.flags
    }

    fn execute_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        let history = History::load_from_disk()?;

        if self.get_flag("--clear").is_some() || self.get_flag("-c").is_some() {
            let mut history = History::load_from_disk()?;
            history.clear();
            history.save();
            return Ok(());
        }

        for command in history {
            println!("{}", command);
        }
        Ok(())
    }

    fn get_help(&self) -> CommandHelp {
        CommandHelp {
            short_desc: "Display or manage the command history".to_string(),
            long_desc: "Show the history of commands that have been executed. \
                       You can also clear the history using the --clear flag.".to_string(),
            usage: "history [flags]".to_string(),
            flags: vec![
                ("--help, -h".to_string(), "Show this help message".to_string()),
                ("--clear, -c".to_string(), "Clear the command history".to_string()),
            ],
        }
    }
}

pub struct SystemCommand {
    pub name: String,
    pub args: Vec<String>,
    pub flags: Vec<Flag>,
    pub io_redirection: IoRedirection,
}

impl SystemCommand {
    pub fn new(name: String) -> Self {
        Self {
            name,
            args: vec![],
            flags: vec![],
            io_redirection: IoRedirection::default(),
        }
    }
}

impl Command for SystemCommand {
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

    fn get_io_redirection(&mut self) -> &mut IoRedirection {
        &mut self.io_redirection
    }

    fn execute_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut command = std::process::Command::new(&self.name);
        
        command.args(&self.args);
        
        for flag in &self.flags {
            if let Some(value) = &flag.value {
                command.arg(flag.ident.to_string() + "=" + value);
            } else {
                command.arg(flag.ident.to_string());
            }
        }

        let output = command.output()?;
        
        if !output.stdout.is_empty() {
            std::io::stdout().write_all(&output.stdout)?;
        }
        
        if !output.stderr.is_empty() {
            std::io::stderr().write_all(&output.stderr)?;
        }

        if !output.status.success() {
            return Err(format!("Command '{}' failed with exit code: {}", 
                self.name, 
                output.status.code().unwrap_or(-1))
                .into());
        }

        Ok(())
    }

    fn get_help(&self) -> CommandHelp {
        CommandHelp {
            short_desc: format!("Execute the system command '{}'", self.name),
            long_desc: format!("Execute the system command '{}' with the provided arguments and flags.", self.name),
            usage: format!("{} [flags] [args...]", self.name),
            flags: vec![
                ("--help, -h".to_string(), "Show this help message".to_string()),
            ],
        }
    }
}

pub struct Pipeline {
    commands: Vec<Box<dyn Command>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    pub fn add_command(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    pub fn execute(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.commands.is_empty() {
            return Ok(());
        }

        // Single command case - no pipes needed
        if self.commands.len() == 1 {
            return self.commands[0].execute();
        }

        // Multiple commands case - need to set up pipes
        use std::process::{Command as ProcessCommand, Stdio};
        use std::io::{Read, Write};
        
        let mut previous_output: Option<Vec<u8>> = None;
        let command_count = self.commands.len();
        
        // Process each command in the pipeline
        for i in 0..command_count {
            let is_last = i == command_count - 1;
            
            // Take ownership of the command temporarily
            let mut cmd = std::mem::replace(&mut self.commands[i], Box::new(SystemCommand::new("dummy".to_string())));
            
            match cmd.get_name() {
                // Handle built-in commands
                "cd" | "pwd" | "history" => {
                    // Create pipes for I/O
                    let mut process = ProcessCommand::new("sh");
                    process.arg("-c").arg(format!("{} {}", cmd.get_name(), cmd.get_args().join(" ")));
                    
                    // Set up stdin from previous command's output
                    if let Some(data) = previous_output.take() {
                        let mut child = process.stdin(Stdio::piped()).spawn()?;
                        if let Some(mut stdin) = child.stdin.take() {
                            stdin.write_all(&data)?;
                        }
                        process = ProcessCommand::new("sh");
                        process.arg("-c").arg(format!("{} {}", cmd.get_name(), cmd.get_args().join(" ")));
                    }
                    
                    // Set up stdout pipe if not the last command
                    if !is_last {
                        process.stdout(Stdio::piped());
                    }
                    
                    // Spawn the process
                    let mut child = process.spawn()?;
                    
                    // Save stdout for the next command if not the last
                    if !is_last {
                        let mut buffer = Vec::new();
                        if let Some(mut stdout) = child.stdout.take() {
                            stdout.read_to_end(&mut buffer)?;
                            previous_output = Some(buffer);
                        }
                    }
                    
                    // Wait for the process to complete
                    let status = child.wait()?;
                    if !status.success() {
                        return Err(format!("Command failed with status: {}", status).into());
                    }
                }
                
                // Handle system commands
                _ => {
                    let mut process = ProcessCommand::new(cmd.get_name());
                    process.args(cmd.get_args());
                    
                    // Set up stdin from previous command's output
                    if let Some(data) = previous_output.take() {
                        let mut child = process.stdin(Stdio::piped()).spawn()?;
                        if let Some(mut stdin) = child.stdin.take() {
                            stdin.write_all(&data)?;
                        }
                        process = ProcessCommand::new(cmd.get_name());
                        process.args(cmd.get_args());
                    }
                    
                    // Set up stdout pipe if not the last command
                    if !is_last {
                        process.stdout(Stdio::piped());
                    }
                    
                    // Spawn the process
                    let mut child = process.spawn()?;
                    
                    // Save stdout for the next command if not the last
                    if !is_last {
                        let mut buffer = Vec::new();
                        if let Some(mut stdout) = child.stdout.take() {
                            stdout.read_to_end(&mut buffer)?;
                            previous_output = Some(buffer);
                        }
                    }
                    
                    // Wait for the process to complete
                    let status = child.wait()?;
                    if !status.success() {
                        return Err(format!("Command failed with status: {}", status).into());
                    }
                }
            }
            
            // Restore the command
            self.commands[i] = cmd;
        }

        Ok(())
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
        let cmd = parser.parse().unwrap();
        
        // Verify it's treated as a system command
        assert_eq!(cmd.get_name(), "unknown_cmd");
        assert_eq!(cmd.get_args(), &["arg1", "arg2"]);
        assert!(cmd.get_flags().is_empty());
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

    #[test]
    fn test_command_get_args_len() {
        let mut cmd = ChangeDirCommand::new();
        cmd.get_args_mut().push("arg1".to_string());
        cmd.get_args_mut().push("arg2".to_string());
        assert_eq!(cmd.get_args_len(), 2);
    }

    #[test]
    fn test_pipeline_parsing() {
        let tokens = create_tokens("ls -l | grep test");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        // Verify it's a pipeline
        assert_eq!(cmd.get_name(), "pipeline");
    }

    #[test]
    fn test_pipeline_execution() {
        let tokens = create_tokens("echo 'Hello, World!' | grep Hello");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        // Execute the pipeline
        let result = cmd.execute();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_with_invalid_command() {
        let tokens = create_tokens("ls -l | nonexistent_command");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        // Execute should fail because of the nonexistent command
        let result = cmd.execute();
        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_with_echo() {
        let tokens = create_tokens("echo test | grep test");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        // Execute the pipeline
        let result = cmd.execute();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_with_cat() {
        use std::fs::File;
        use std::io::Write;
        
        // Create a test file
        let mut file = File::create("test.txt").unwrap();
        writeln!(file, "test line 1\ntest line 2").unwrap();
        
        let tokens = create_tokens("cat test.txt | grep line");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        // Execute the pipeline
        let result = cmd.execute();
        
        // Clean up
        std::fs::remove_file("test.txt").unwrap();
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_with_multiple_commands() {
        let tokens = create_tokens("echo 'line 1\nline 2\nline 3' | grep line | wc -l");
        let mut parser = CommandParser::new(tokens);
        let cmd = parser.parse().unwrap();
        
        // Execute the pipeline
        let result = cmd.execute();
        assert!(result.is_ok());
    }
}

