use shell::Shell;

fn main() {
    let mut shell = Shell::new();
    shell.eval().unwrap();
}
