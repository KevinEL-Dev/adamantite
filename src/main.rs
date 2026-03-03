use chrono::{TimeDelta, prelude::*};
use clap::{Parser, Subcommand, ValueEnum};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::time::{Duration,Instant};
use std::{error::Error};
use std::process;
use std::path::Path;
use std::process::{Command, Stdio};
use serde::Serialize;
use sysinfo::{Disks, Pid, ProcessRefreshKind, ProcessesToUpdate, System};
const LOW_IO_PRESSURE_MAX: f64 = 1.0;
const MODERATE_IO_PRESSURE_MAX: f64 = 1.0;
const HIGH_IO_PRESSURE_MAX: f64 = 1.0;

/// Track system resources over time
#[derive(Parser)]
#[command(version, about, long_about= None)]
struct Cli {
    #[command[subcommand]]
    command: Option<Commands>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug,Serialize)]
enum SystemResource {
    /// System resource cpu
    Cpu,
    /// System resource memory
    Mem,
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum PressureType {
    /// Pressure type Io
    Io,
    /// Pressure type Mem
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
    /// Shows how often system work is stalled due to resource contention
    Pressure {
        /// The resource to inspect
        #[arg(value_enum)]
        pressure_type: PressureType,
    },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AvgResourceUsage {
    system_resource: SystemResource,
    resource_usage_per_second_map: HashMap<u32, f32>,
}
impl AvgResourceUsage {
    fn init(system_resource: SystemResource) -> AvgResourceUsage {
        let new_map: HashMap<u32, f32> = HashMap::new();
        AvgResourceUsage {
            system_resource,
            resource_usage_per_second_map: new_map,
        }
    }
    fn add_new_entry(&mut self, time_in_seconds: u32, avg_resource_usage: f32) {
        self.resource_usage_per_second_map
            .insert(time_in_seconds, avg_resource_usage);
    }
}
struct Pressure {
    some: HashMap<String, f64>,
    full: HashMap<String, f64>,
    pressure_type: PressureType,
}
impl Pressure {
    fn build_pressure(some: Vec<&str>, full: Vec<&str>, pressure_type: PressureType) -> Pressure {
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
            pressure_type,
        }
    }
    fn evaluate_pressure(&self) {
        let avg300_val = self.some.get("avg300");
        match avg300_val {
            Some(val) => {
                if *val < LOW_IO_PRESSURE_MAX {
                    if self.pressure_type == PressureType::Io {
                        println!("Io pressure is healthy ({}% sustained)", val);
                    } else {
                        println!("Mem pressure is healthy ({}% sustained)", val);
                    }
                } else if *val >= MODERATE_IO_PRESSURE_MAX {
                    println!("Disk pressure is concerning ({}% sustained)", val);
                    if self.pressure_type == PressureType::Io {
                        println!("Io pressure is concerning ({}% sustained)", val);
                    } else {
                        println!("Mem pressure is concerning ({}% sustained)", val);
                    }
                }
            }
            None => println!("failed to find the avg300_val"),
        }
    }
}
fn run (struct_avg: AvgResourceUsage) -> Result<(), Box<dyn Error>> {
    let mut wtr = csv::Writer::from_writer(io::stdout());
    // wtr.serialize(struct_avg)?;

    let size_of_map = struct_avg.resource_usage_per_second_map.len();
    wtr.write_record(["Seconds","Resource_Usage"])?;
    for i in 1..size_of_map+1 {
        let temp = i as u32;
        let resource_usage = struct_avg.resource_usage_per_second_map.get(&temp);
        wtr.serialize((i,resource_usage))?;
    }
    wtr.flush()?;
    Ok(())
}
fn main() {
    // parges user input
    let args = Cli::parse();
    let start_time = Utc::now().time();
    let mut end_time = Utc::now().time();
    let mut sys = System::new();
    let mut diff = end_time - start_time;
    let mut user_selected_time = 1;
    sys.refresh_all();
    let num_of_cpus = sys.cpus().len();
    let hytale_pid = find_pid_of_hytale();
    let num_of_cpus = num_of_cpus as f32;
    /*let disks = Disks::new_with_refreshed_list();
    for disk in disks.list() {
        println!("[{:?}] {:?}", disk.name(), disk.usage());
    }*/
    match &args.command {
        Some(Commands::Track {
            system_resource,
            time_seconds,
        }) => match system_resource {
            SystemResource::Cpu => {
                let mut cpu_avg_resource_usg = AvgResourceUsage::init(SystemResource::Cpu);
                let time_delta = TimeDelta::seconds(*time_seconds);

                let mut last_emit = Instant::now();

                user_selected_time = *time_seconds;
                let mut counter = 0;
                // keeps track of how many times one second has passed
                let mut time_in_seconds_counter: u32 = 0;
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
                    total_system_cpu_usage +=
                        current_system_cpu_usage / total_available_cpu_percentage;

                    let hytale_curr_cpu_usage = get_cpu_usage_from_pid(hytale_pid);

                    hytale_total_cpu_usage += hytale_curr_cpu_usage;
                    // one second has passed
                    if last_emit.elapsed() >= Duration::from_secs(1){
                        time_in_seconds_counter += 1;
                        cpu_avg_resource_usg.add_new_entry(
                            time_in_seconds_counter,
                            ((hytale_total_cpu_usage / counter as f32)
                                / total_available_cpu_percentage)
                                * 100.0,
                        );
                        last_emit += Duration::from_secs(1);
                    }
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
                    ((hytale_total_cpu_usage / counter as f32) / total_available_cpu_percentage)
                        * 100.0
                );
                if let Err(err) = run(cpu_avg_resource_usg){
                    println!("{}", err);
                    process::exit(1);
                }
            }
            SystemResource::Mem => {
                let time_delta = TimeDelta::seconds(*time_seconds);
                user_selected_time = *time_seconds;
                let system_total_memory = sys.total_memory();

                let mut total_mem_usage_in_bytes = 0;
                let mut total_hytale_mem_usage_in_bytes = 0;
                let mut counter = 0;
                while diff < time_delta {
                    // only refresh ram
                    sys.refresh_memory_specifics(
                        sysinfo::MemoryRefreshKind::everything().with_ram(),
                    );
                    end_time = Utc::now().time();
                    diff = end_time - start_time;
                    let curr_system_mem_in_bytes = sys.used_memory();
                    let curr_hytale_mem_in_bytes = get_mem_usage_from_pid(hytale_pid);
                    total_mem_usage_in_bytes += curr_system_mem_in_bytes;
                    total_hytale_mem_usage_in_bytes += curr_hytale_mem_in_bytes;
                    counter += 1;
                }
                let total_mem_in_gigabytes =
                    return_mem_in_gigabytes(total_mem_usage_in_bytes as f64);
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
        },
        Some(Commands::Pressure { pressure_type }) => match pressure_type {
            PressureType::Io => {
                show_system_pressure(PressureType::Io);
            }
            PressureType::Mem => {
                show_system_pressure(PressureType::Mem);
            }
        },
        None => {}
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
fn show_system_pressure(pressure_type: PressureType) {
    let path_to_psi_io = Path::new("/proc/pressure/io");
    let path_to_psi_mem = Path::new("/proc/pressure/memory");

    let mut f = File::open(path_to_psi_io).expect("failed to open this file");
    let mut content_io = String::new();
    f.read_to_string(&mut content_io)
        .expect("failed to read the file");
    let mut f_memory = File::open(path_to_psi_mem).expect("failed to open file memory");
    let mut content_memory = String::new();
    f_memory
        .read_to_string(&mut content_memory)
        .expect("failed to read file memory");

    let v_f: Vec<&str> = content_io.lines().collect();
    let v_f_some: Vec<&str> = v_f[0].split_whitespace().collect();
    let v_f_full: Vec<&str> = v_f[1].split_whitespace().collect();

    let v_f_mem: Vec<&str> = content_memory.lines().collect();
    let v_f_mem_some: Vec<&str> = v_f_mem[0].split_whitespace().collect();
    let v_f_mem_full: Vec<&str> = v_f_mem[1].split_whitespace().collect();

    match pressure_type {
        PressureType::Io => {
            let pressure_example = Pressure::build_pressure(v_f_some, v_f_full, PressureType::Io);
            pressure_example.evaluate_pressure();
        }
        PressureType::Mem => {
            let pressure_example =
                Pressure::build_pressure(v_f_mem_some, v_f_mem_full, PressureType::Mem);
            pressure_example.evaluate_pressure();
        }
    }
}
