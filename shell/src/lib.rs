use std::io::Write;

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
                self.put_line(&line);
                // let tokens = tokenize(line);
                //let ast = parse(tokens);
                //let result = eval(ast);
                //println!("{}", result);
            } else {
                self.put_prefixed_line("");
            }
        }
    }

    fn read_line(&self) -> Option<String> { 
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok()?;
        Some(line)
    }
}