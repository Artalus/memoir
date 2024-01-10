extern crate memoir;

use std::env;

pub fn main() -> anyhow::Result<()>{
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        memoir::control::run_control(args[1..].to_vec())
    } else {
        // x20 is ansi space, used to keep 4 spaces in each line
        println!("Memoir is a small tool to monitor current RAM consumption on per-process basis\n\
        Usage:\n\
        \x20   {0} once        - get current RAM info, print and exit\n\
        \x20   {0} start       - start a daemon to monitor RAM\n\
        \x20   {0} stop        - stop a running daemon \n\
        \x20   {0} ping        - check if daemon is running\n\
        \x20   {0} save <path> -
        ", args[0]);
        Ok(())
    }
}
