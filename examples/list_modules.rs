//! Print every registered module name, for auditing remote-transport coverage.

fn main() {
    let registry = rustible::modules::ModuleRegistry::with_builtins();
    let mut names: Vec<&str> = registry.names();
    names.sort_unstable();
    for name in names {
        println!("{}", name);
    }
}
