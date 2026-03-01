use chrono::{TimeDelta, prelude::*};
use clap::{Parser, Subcommand, ValueEnum};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};
use sysinfo::{Disks, Pid, ProcessRefreshKind, ProcessesToUpdate, System};
/// Track system resources over time
#[derive(Parser)]
#[command(version, about, long_about= None)]
struct Cli {
    #[command[subcommand]]
    command: Option<Commands>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SystemResource {
    /// System resource cpu
    Cpu,
    /// System resource memory
    Mem,
}
#[derive(Subcommand)]
enum Commands {
    /// Tracks a system resource
    Track {
        #[arg(value_enum)]
        system_resource: SystemResource,
        #[arg(short, long, default_value_t = 1)]
        time_seconds: i64,
    },
}
struct Pressure {
    some: HashMap<String, f64>,
    full: HashMap<String, f64>,
}
impl Pressure {
    fn build_pressure(some: Vec<&str>, full: Vec<&str>) -> Pressure {
        let mut map_some: HashMap<String, f64> = HashMap::new();
        let mut map_full: HashMap<String, f64> = HashMap::new();

        let mut split_test_arg: Vec<&str> = some[1].split("=").collect();
        for arg in some.iter().skip(1) {
            /* first value is avg then percentage*/
            /* so set first value to string name and second to f64 by parsing the string into f64*/
            split_test_arg = arg.split("=").collect();
            let avg_second = split_test_arg[0];
            let avg_percentage: f64 = split_test_arg[1]
                .parse::<f64>()
                .expect("didnt see a float value");
            /* insert avg_second and avg_percentage as keys into some hashmap and then full hashmap*/
            map_some.insert(avg_second.to_string(), avg_percentage);
        }
        let mut split_test_arg: Vec<&str> = full[1].split("=").collect();
        for arg in full.iter().skip(1) {
            /* first value is avg then percentage*/
            /* so set first value to string name and second to f64 by parsing the string into f64*/
            split_test_arg = arg.split("=").collect();
            let avg_second = split_test_arg[0];
            let avg_percentage: f64 = split_test_arg[1]
                .parse::<f64>()
                .expect("didnt see a float value");
            /* insert avg_second and avg_percentage as keys into some hashmap and then full hashmap*/
            map_full.insert(avg_second.to_string(), avg_percentage);
        }
        Pressure {
            some: map_some,
            full: map_full,
        }
    }
}
fn main() {
    // parges user input
    let args = Cli::parse();
    let start_time = Utc::now().time();
    let mut end_time = Utc::now().time();
    let mut sys = System::new();
    let mut diff = end_time - start_time;
    let mut user_selected_time = 1;
    let mut time_delta = TimeDelta::seconds(user_selected_time);
    let mut sys_resource = SystemResource::Cpu;
    /*let disks = Disks::new_with_refreshed_list();
    for disk in disks.list() {
        println!("[{:?}] {:?}", disk.name(), disk.usage());
    }*/
    show_system_pressure();
    match &args.command {
        Some(Commands::Track {
            system_resource,
            time_seconds,
        }) => match system_resource {
            SystemResource::Cpu => {
                time_delta = TimeDelta::seconds(*time_seconds);
                sys_resource = *system_resource;
                user_selected_time = *time_seconds;
            }
            SystemResource::Mem => {
                time_delta = TimeDelta::seconds(*time_seconds);
                sys_resource = *system_resource;
                user_selected_time = *time_seconds;
            }
        },
        None => {}
    }

    sys.refresh_all();
    let num_of_cpus = sys.cpus().len();
    let hytale_pid = find_pid_of_hytale();
    let num_of_cpus = num_of_cpus as f32;
    if sys_resource == SystemResource::Cpu {
        let mut counter = 0;
        let mut hytale_total_cpu_usage: f32 = 0.0;
        let mut total_system_cpu_usage: f32 = 0.0;
        let total_available_cpu_percentage: f32 = num_of_cpus * 100.0;
        while diff < time_delta {
            sys.refresh_cpu_usage();
            let mut current_system_cpu_usage: f32 = 0.0;
            end_time = Utc::now().time();
            diff = end_time - start_time;
            for cpu in sys.cpus() {
                current_system_cpu_usage += cpu.cpu_usage();
            }
            total_system_cpu_usage += current_system_cpu_usage / total_available_cpu_percentage;

            let hytale_curr_cpu_usage = get_cpu_usage_from_pid(hytale_pid);

            hytale_total_cpu_usage += hytale_curr_cpu_usage;
            counter += 1;
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        }
        println!(
            "For {} seconds hytale on average used {:.2} cpus",
            user_selected_time,
            ((hytale_total_cpu_usage / counter as f32) / total_available_cpu_percentage)
                * num_of_cpus
        );
        println!(
            "For {} seconds your system on average used {:.2} cpus",
            user_selected_time,
            (total_system_cpu_usage / counter as f32) * (num_of_cpus)
        );
        println!(
            "For {} seconds your system on average was at {:.2}% load",
            user_selected_time,
            (total_system_cpu_usage / counter as f32) * 100.0
        );
        println!(
            "For {} seconds hytale on average used {:.2}% load",
            user_selected_time,
            ((hytale_total_cpu_usage / counter as f32) / total_available_cpu_percentage) * 100.0
        );
    } else if sys_resource == SystemResource::Mem {
        let system_total_memory = sys.total_memory();

        let mut total_mem_usage_in_bytes = 0;
        let mut total_hytale_mem_usage_in_bytes = 0;
        let mut counter = 0;
        while diff < time_delta {
            // only refresh ram
            sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::everything().with_ram());
            end_time = Utc::now().time();
            diff = end_time - start_time;
            let curr_system_mem_in_bytes = sys.used_memory();
            let curr_hytale_mem_in_bytes = get_mem_usage_from_pid(hytale_pid);
            total_mem_usage_in_bytes += curr_system_mem_in_bytes;
            total_hytale_mem_usage_in_bytes += curr_hytale_mem_in_bytes;
            counter += 1;
        }
        let total_mem_in_gigabytes = return_mem_in_gigabytes(total_mem_usage_in_bytes as f64);
        let total_hytale_mem_in_gigabytes =
            return_mem_in_gigabytes(total_hytale_mem_usage_in_bytes as f64);
        let average_hytale_mem_usage_gb = total_hytale_mem_in_gigabytes / counter as f64;

        println!(
            "Average mem usage of hytale over {} seconds is {:.2} gb.",
            user_selected_time, average_hytale_mem_usage_gb
        );
        let average_system_mem_usage_gb = total_mem_in_gigabytes / counter as f64;

        println!(
            "Average mem usage of entire system over {} seconds is {:.2} gb.",
            user_selected_time, average_system_mem_usage_gb
        );
        let average_hytale_mem_usage = return_mem_usage(
            total_hytale_mem_usage_in_bytes as f64 / counter as f64,
            system_total_memory as f64,
        );
        println!(
            "Average mem usage in percentage for hytale over {} seconds is {:.2}%",
            user_selected_time, average_hytale_mem_usage
        );
        let average_system_mem_usage = return_mem_usage(
            total_mem_usage_in_bytes as f64 / counter as f64,
            system_total_memory as f64,
        );
        println!(
            "Average mem usage in percentage for entire system over {} seconds is {:.2}%",
            user_selected_time, average_system_mem_usage
        );
        println!(
            "Average mem usage in percentage for entire system without hytale processes over {} seconds is {:.2}%",
            user_selected_time,
            average_system_mem_usage - average_hytale_mem_usage
        );
    }
    /* else if args.system_resource == "net" {
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
    }*/
    else {
        println!(
            "Please input a valid system resource. Only current valid resources are \"cpu\" and \"mem\""
        );
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
fn get_cpu_usage_from_pid(pid: u32) -> f32 {
    let mut s = System::new_all();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    s.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu(),
    );
    let mut total_cpu_usage: f32 = 0.0;
    if let Some(process) = s.process(Pid::from_u32(pid)) {
        if let Some(tasks) = process.tasks() {
            for task_pid in tasks {
                if let Some(task) = s.process(*task_pid) {
                    let curr_cpu_usage = task.cpu_usage();
                    total_cpu_usage += curr_cpu_usage;
                }
            }
        }
    }
    return total_cpu_usage;
}
fn get_mem_usage_from_pid(pid: u32) -> u64 {
    let s = System::new_all();
    let mut pid_mem_usage: u64 = 0;
    if let Some(process) = s.process(Pid::from_u32(pid)) {
        pid_mem_usage = process.memory();
    }
    return pid_mem_usage;
}
fn return_mem_in_gigabytes(mem_in_bytes: f64) -> f64 {
    let kb: u64 = 1000;
    let divisor = num::pow(kb, 3);
    (mem_in_bytes) / (divisor as f64)
}
fn return_mem_usage(mem_in_bytes: f64, system_total_mem_in_bytes: f64) -> f64 {
    (mem_in_bytes / system_total_mem_in_bytes) * 100.0
}
fn show_system_pressure() {
    let path_to_psi_io = Path::new("/proc/pressure/io");
    let path_to_psi_mem = Path::new("/proc/pressure/memory");

    let mut f = File::open(path_to_psi_io).expect("failed to open this file");
    let mut content = String::new();
    f.read_to_string(&mut content)
        .expect("failed to read the file");
    let mut f_memory = File::open(path_to_psi_mem).expect("failed to open file memory");
    let mut content_memory = String::new();
    f_memory
        .read_to_string(&mut content_memory)
        .expect("failed to read file memory");

    let v_f: Vec<&str> = content.lines().collect();
    let v_f_some: Vec<&str> = v_f[0].split_whitespace().collect();
    let v_f_full: Vec<&str> = v_f[1].split_whitespace().collect();

    let pressure_example = Pressure::build_pressure(v_f_some, v_f_full);
    println!("hashmap generated for some");
    for (key, value) in pressure_example.some.iter() {
        println!("key: {key} val: {value}");
    }
    println!("hashmap generated for full");
    for (key, value) in pressure_example.some.iter() {
        println!("key: {key} val: {value}");
    }
}
