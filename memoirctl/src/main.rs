extern crate memoir;

// fn main() {
//     let result = memoir::add(1, 2);
//     println!("Hello, world {}!", result);
// }

use std::env;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    for a in &args {
        println!("arg {}", a);
    }

    if args.len() > 1 && args[1] == "memoirexe" {
        run_memoirexe();
    } else if args.len() > 1 && args[1] == "memoirctl" {
        run_memoirctl(args);
    } else {
        println!("Usage: {} [memoirexe|memoirctl]", args[0]);
    }
}

fn run_memoirexe() {
    println!("memoirexe is running...");
    memoir::daemon::run_daemon();
    println!("memoirexe finished");
}

fn run_memoirctl(args: Vec<String>) {
    println!("memoirctl is running...");
    if args.len() <= 2 {
        println!("Usage: {} memoirctl <stop|>", args[0]);
        return;
    }
    memoir::control::run_control(args[2..].to_vec());
    println!("memoirctl finished");
}
