use shell::Shell;

fn main() {
    let shell = Shell::new();
    shell.eval().unwrap();
}
