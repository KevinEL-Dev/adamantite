use chrono::{TimeDelta, prelude::*};
use clap::Parser;
use std::process::{Command, Stdio};
use sysinfo::{Networks, System};
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
    } else if args.system_resource == "net" {
        let mut networks = Networks::new_with_refreshed_list();
        println!("=> networks:");
        while diff < time_delta {
            networks.refresh(true);
            end_time = Utc::now().time();
            diff = end_time - start_time;
            for (interface_name, data) in &networks {
                println!(
                    "{interface_name}: {} B (down) / {} B (up)",
                    data.total_received(),
                    data.total_transmitted(),
                );
            }
        }
    }

    find_pid_of_hytale();
}
fn find_pid_of_hytale() {
    let ps_child = Command::new("/bin/ps")
        .arg("aux")
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start ps");
    let ps_out = ps_child.stdout.expect("failed to start echo process");

    let mut grep_child = Command::new("/bin/grep")
        .arg("java -XX:AOTCache=HytaleServer.aot")
        .stdin(Stdio::from(ps_out))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed te start grep process");

    let grep_output = grep_child.stdout.expect("failed to get grep output");

    let mut head_child = Command::new("/bin/head")
        .arg("-n")
        .arg("1")
        .stdin(Stdio::from(grep_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start the head process");
    let head_output = head_child.stdout.expect("failed to get head output");

    /*let output = head_child
        .wait_with_output()
        .expect("failed to wait for head");
    println!(
        "output from head\n {}",
        String::from_utf8_lossy(&output.stdout)
    );*/

    let awk_child = Command::new("/bin/awk")
        .arg("{print $2}")
        .stdin(Stdio::from(head_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start head process");

    let output = awk_child
        .wait_with_output()
        .expect("failed to wait for head");
    println!(
        "output from awk\n {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
