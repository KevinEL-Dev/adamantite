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
    let start_time = Utc::now().time();
    let mut end_time = Utc::now().time();
    let mut sys = System::new();
    let mut diff = end_time - start_time;
    let time_delta = TimeDelta::seconds(args.time_seconds);

    sys.refresh_all();
    let num_of_cpus = sys.cpus().len();
    let hytale_pid = find_pid_of_hytale();
    let num_of_cpus = num_of_cpus as f32;

    if args.system_resource == "cpu" {
        let mut counter = 0;
        let mut total_cpu_usage: f32 = 0.0;
        let mut total_system_cpu_usage: f32 = 0.0;
        let total_available_cpu_percentage: f32 = num_of_cpus * 100.0;
        while diff < time_delta {
            sys.refresh_cpu_usage();
            let mut current_system_cpu_usage: f32 = 0.0;
            end_time = Utc::now().time();
            diff = end_time - start_time;
            for cpu in sys.cpus() {
                println!("{}%", cpu.cpu_usage());
                current_system_cpu_usage += cpu.cpu_usage();
            }
            println!(
                "avg number of cores being used {}",
                (current_system_cpu_usage / total_available_cpu_percentage) * num_of_cpus
            );
            total_system_cpu_usage +=
                (current_system_cpu_usage / total_system_cpu_usage) * num_of_cpus;

            let curr_cpu_usage = get_hytale_total_cpu_usage(hytale_pid);

            /*println!("hytale curr cpu usage is {}%", curr_cpu_usage);*/
            total_cpu_usage += curr_cpu_usage;
            counter += 1;
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        }
        println!(
            "for {} seconds hytale on average used {} cpus",
            args.time_seconds,
            (total_cpu_usage / counter as f32) / (100.0 * num_of_cpus)
        );
        println!(
            "for {} seconds your system on avgerage used {} cpus)",
            args.time_seconds,
            (total_cpu_usage / counter as f32)
        );
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
}
fn find_pid_of_hytale() -> u32 {
    let ps_child = Command::new("/bin/ps")
        .arg("aux")
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start ps");
    let ps_out = ps_child.stdout.expect("failed to start echo process");

    let grep_child = Command::new("/bin/grep")
        .arg("java -jar HytaleServer.jar --assets ../Assets.zip --backup --backup-dir backups --backup-frequency 30")
        .stdin(Stdio::from(ps_out))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed te start grep process");

    let grep_output = grep_child.stdout.expect("failed to get grep output");

    let head_child = Command::new("/bin/head")
        .arg("-n")
        .arg("1")
        .stdin(Stdio::from(grep_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start the head process");
    let head_output = head_child.stdout.expect("failed to get head output");

    let awk_child = Command::new("/bin/awk")
        .arg("{print $2}")
        .stdin(Stdio::from(head_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start awk process");

    let output = awk_child
        .wait_with_output()
        .expect("failed to wait for awk");
    let s = String::from_utf8_lossy(&output.stdout).to_string();
    let pid_from_s: u32 = s.trim().parse().expect("not a valid number");
    return pid_from_s;
}
fn get_hytale_total_cpu_usage(hytale_pid: u32) -> f32 {
    let result_arg_top = format!("{}", hytale_pid);
    let top_child = Command::new("/bin/top")
        .arg("-b")
        .arg("-n")
        .arg("1")
        .arg("-p")
        .arg(result_arg_top)
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start ps");
    let top_output = top_child.stdout.expect("failed to start top process");
    let result_arg = format!(
        "$1 == \"PID\" {{block_num++; next}} block_num == 1 {{sum += $9;}} END {{print sum}}"
    );

    let awk_child = Command::new("/bin/awk")
        .arg(result_arg)
        .stdin(Stdio::from(top_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start awk process");
    let output = awk_child
        .wait_with_output()
        .expect("failed to wait for awk");
    /*println!(
        "output from awk\ncpu usage is {}%",
        String::from_utf8_lossy(&output.stdout).trim()
    );*/
    let s = String::from_utf8_lossy(&output.stdout).to_string();
    let hytale_cpu_usage: f32 = s.trim().parse().expect("not a valid number");
    return hytale_cpu_usage;
}
