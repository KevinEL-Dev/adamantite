use chrono::{TimeDelta, prelude::*};
use clap::Parser;
use sysinfo::System;

#[derive(Parser)]
#[command(version, about, long_about= None)]
struct Cli {
    // type should be the cpu
    #[arg(short, long)]
    system_resource: String,

    #[arg(short, long, default_value_t = 1)]
    time_seconds: i64,
}
fn main() {
    let args = Cli::parse();
    println!(
        "pattern: {:?}, path: {:?}",
        args.system_resource, args.time_seconds
    );
    let start_time = Utc::now().time();
    let mut end_time = Utc::now().time();
    let mut sys = System::new();
    let mut diff = end_time - start_time;
    let time_delta = TimeDelta::seconds(args.time_seconds);
    if args.system_resource == "cpu" {
        sys.refresh_cpu_usage();
        while diff < time_delta {
            end_time = Utc::now().time();
            diff = end_time - start_time;
            for cpu in sys.cpus() {
                println!("{}%", cpu.cpu_usage());
            }
        }
    }
}
