use chrono::{TimeDelta, prelude::*};
use clap::Parser;
use num::pow;
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
    // parges user input
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
        while diff < time_delta {
            sys.refresh_cpu_usage();
            end_time = Utc::now().time();
            diff = end_time - start_time;
            for cpu in sys.cpus() {
                println!("{}%", cpu.cpu_usage());
            }
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        }
    } else if args.system_resource == "mem" {
        while diff < time_delta {
            // only refresh ram
            sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::everything().with_ram());
            end_time = Utc::now().time();
            diff = end_time - start_time;
            let mem_in_bytes = sys.used_memory();
            let kb: u64 = 1000;
            let divisor = num::pow(kb, 3);

            // to get decimal on prints
            let mem_in_gigabytes = (mem_in_bytes as f64) / (divisor as f64);
            println!("mem usage {} gb", mem_in_gigabytes);
        }
    }
}
