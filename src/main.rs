use chrono::{TimeDelta, prelude::*};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::process;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use config::{Map,Value,Config};

use directories::ProjectDirs;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::prelude::*;
use ratatui::style::Style;
use ratatui::widgets::{Bar, BarChart};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{
        Block, List, ListDirection, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Widget,
    },
};
const LOW_IO_PRESSURE_MAX: f64 = 1.0;
const MODERATE_IO_PRESSURE_MAX: f64 = 1.0;
const HIGH_IO_PRESSURE_MAX: f64 = 1.0;
#[derive(Debug)]
enum CustomError {
    NoJournalCtlOutput,
}
impl Error for CustomError {}

impl Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NoJournalCtlOutput => "Error: No entry for journalctl",
        };
        write!(f, "Error: {message}")
    }
}
/// Track system resources over time
#[derive(Parser)]
#[command(version, about, long_about= None)]
struct Cli {
    #[command[subcommand]]
    command: Option<Commands>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Serialize)]
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
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputType {
    /// Output type of CSV
    Csv,
    /// Default output type is None
    None,
}
#[derive(Subcommand)]
enum Commands {
    /// Tracks a system resource
    Track {
        #[arg(value_enum)]
        system_resource: SystemResource,
        #[arg(short, long, default_value_t = 1)]
        time_seconds: i64,
        #[arg(short, long,value_enum,default_value_t = OutputType::None)]
        output: OutputType,
    },
    /// Shows how often system work is stalled due to resource contention
    Pressure {
        /// The resource to inspect
        #[arg(value_enum)]
        pressure_type: PressureType,
    },
    /// Shows Live Hytale vs System metrics
    Live {
        /// Interval update time in ms
        #[arg(short, long, default_value_t = 500)]
        interval_ms: u64,
    },
}

#[cfg(test)]
mod tests {

    use super::*;
    use ratatui::style::Style;

    #[test]
    fn handle_key_event() {
        let mut app = App::default();
        app.handle_key_event(KeyCode::Char('q').into());
        assert!(app.exit);
    }
}
#[derive(Debug, Default)]
pub struct App {
    system_cpu_usage: f32,
    hytale_cpu_usage: f32,
    system_mem_usage: f64,
    hytale_mem_usage: f64,
    current_hytale_log: Vec<HytaleLog>,
    current_hytale_log_strings: Vec<String>,
    vertical_scroll: usize,
    vertical: ScrollbarState,
    curr_index: i64,
    exit: bool,
}
impl App {
    pub fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        interval_ms: i64,
        sys: &mut System,
        hytale_pid: u32,
    ) -> io::Result<()> {
        let num_of_cpus = sys.cpus().len() as f32;
        let mut last_emit = Instant::now();
        let mut state = ListState::default();
        self.curr_index = 0;
        let mut vertical = ScrollbarState::new(self.current_hytale_log_strings.len()).position(0);

        while !self.exit {
            if last_emit.elapsed() >= Duration::from_millis(interval_ms.try_into().unwrap()) {
                sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::everything().with_ram());

                sys.refresh_cpu_usage();
                let total_available_cpu_percentage: f32 = num_of_cpus * 100.0;
                let hytale_curr_cpu_usage = get_cpu_usage_from_pid(hytale_pid);
                let curr_system_mem_in_bytes = sys.used_memory();
                let curr_hytale_mem_in_bytes = get_mem_usage_from_pid(hytale_pid);
                let curr_hytale_mem_in_gigabytes =
                    return_mem_in_gigabytes(curr_hytale_mem_in_bytes as f64);
                let curr_system_mem_in_gigabytes =
                    return_mem_in_gigabytes(curr_system_mem_in_bytes as f64);

                let mut current_system_cpu_usage: f32 = 0.0;
                for cpu in sys.cpus() {
                    current_system_cpu_usage += cpu.cpu_usage();
                }
                let current_system_cpu_usage =
                    (current_system_cpu_usage / total_available_cpu_percentage) * 100.0;
                self.update_sys_cpu_usage(current_system_cpu_usage);
                self.update_hytale_cpu_usage(
                    (hytale_curr_cpu_usage / total_available_cpu_percentage) * 100.0,
                );
                let hytale_log = get_hytale_logs();
                let array_of_hytale_log = match get_test_journalctl_output(hytale_log) {
                    Ok(arr) => arr,
                    Err(error) => [].to_vec(),
                };
                self.update_sys_mem_usage(curr_system_mem_in_gigabytes);
                self.update_hytale_mem_usage(curr_hytale_mem_in_gigabytes);
                self.update_current_hytale_log(array_of_hytale_log);
                self.convert_hytale_log_to_array_of_strings();
                vertical = ScrollbarState::new(self.current_hytale_log_strings.len())
                    .position(self.curr_index.try_into().unwrap());
                self.update_current_vertical(vertical);
                last_emit += Duration::from_millis(interval_ms.try_into().unwrap());
            }
            terminal.draw(|frame| self.draw(frame, &mut vertical, &mut state))?;
            if event::poll(Duration::from_millis(16))? {
                self.handle_events(&mut state)?;
            }
        }
        Ok(())
    }
    fn draw(&self, frame: &mut Frame, vertical: &mut ScrollbarState, state: &mut ListState) {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(frame.area());
        let inner_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[1]);
        let list_widget = List::new(self.current_hytale_log_strings.clone())
            .block(Block::bordered().title("List"))
            .style(Style::new().white())
            .highlight_style(Style::new().italic())
            .highlight_symbol(">>")
            .repeat_highlight_symbol(true)
            .direction(ListDirection::TopToBottom);
        frame.render_widget(self, layout[0]);
        frame.render_widget(
            BarChart::new([
                Bar::with_label("system cpu usage", self.system_cpu_usage as u64),
                Bar::with_label("hytale cpu usage", self.hytale_cpu_usage as u64),
                Bar::with_label("system mem usage", self.system_mem_usage as u64),
                Bar::with_label("hytale mem usage", self.hytale_mem_usage as u64),
            ])
            .max(100)
            .block(Block::bordered().title("Live Chart"))
            .bar_width(25)
            .bar_style(Style::new().green())
            .value_style(Style::new().red().bold())
            .label_style(Style::new().white())
            .bar_gap(1),
            inner_layout[1],
        );
        frame.render_stateful_widget(list_widget, inner_layout[0], state);
        frame.render_stateful_widget(
            scrollbar,
            inner_layout[0].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            vertical,
        );
    }
    fn handle_events(&mut self, state: &mut ListState) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event, state)
            }
            _ => {}
        };
        Ok(())
    }
    fn handle_key_event(&mut self, key_event: KeyEvent, state: &mut ListState) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('j') => {
                self.increment_current_scroll();
                self.increment_current_index();
                state.select(Some(self.curr_index.try_into().unwrap()));
            }
            KeyCode::Char('k') => {
                self.deincrement_current_scroll();
                self.deincrement_current_index();
                state.select(Some(self.curr_index.try_into().unwrap()));
            }
            _ => {}
        }
    }
    fn exit(&mut self) {
        self.exit = true;
    }
    fn update_sys_cpu_usage(&mut self, curr_sys_usage: f32) {
        self.system_cpu_usage = curr_sys_usage;
    }
    fn update_hytale_cpu_usage(&mut self, curr_hytale_cpu_usage: f32) {
        self.hytale_cpu_usage = curr_hytale_cpu_usage;
    }
    fn update_sys_mem_usage(&mut self, curr_mem_usage: f64) {
        self.system_mem_usage = curr_mem_usage;
    }
    fn update_hytale_mem_usage(&mut self, curr_hytale_mem_usage: f64) {
        self.hytale_mem_usage = curr_hytale_mem_usage;
    }
    fn update_current_hytale_log(&mut self, curr_hytale_logs: Vec<HytaleLog>) {
        self.current_hytale_log = curr_hytale_logs;
    }
    fn convert_hytale_log_to_array_of_strings(&mut self) {
        for hytale_log in &self.current_hytale_log {
            let local_time = Local::now();
            let formatted_time = local_time.format("%Y-%m-%d %H:%M:%S").to_string();
            let string_curr_sys_usage = String::from(" ")
                + "CPU: "
                + &self.hytale_cpu_usage.to_string()
                + "%"
                + " "
                + &formatted_time;

            self.current_hytale_log_strings
                .push(hytale_log.who.clone() + &string_curr_sys_usage);
        }
    }
    fn update_current_vertical(&mut self, new_vertical: ScrollbarState) {
        self.vertical = new_vertical;
    }
    fn deincrement_current_index(&mut self) {
        if self.curr_index > 0 {
            self.curr_index -= 1
        } else {
            self.curr_index = 0
        }
    }
    fn increment_current_index(&mut self) {
        if self.curr_index < self.current_hytale_log_strings.len().try_into().unwrap() {
            self.curr_index += 1
        }
    }
    fn deincrement_current_scroll(&mut self) {
        if self.vertical_scroll > 0 {
            self.vertical_scroll -= 1
        } else {
            self.curr_index = 0
        }
    }
    fn increment_current_scroll(&mut self) {
        if self.vertical_scroll < self.current_hytale_log_strings.len() {
            self.vertical_scroll += 1
        }
    }
}
impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("adamantite live".bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);
        let counter_text = Text::from(vec![Line::from(vec![
            "System CPU Usage: ".into(),
            self.system_cpu_usage.to_string().yellow(),
            "%".into(),
            " ".into(),
            "Hytale CPU Usage: ".into(),
            self.hytale_cpu_usage.to_string().yellow(),
            "%".into(),
            " ".into(),
            "System Mem Usage: ".into(),
            self.system_mem_usage.to_string().yellow(),
            " GB".into(),
            " ".into(),
            "Hytale Mem Usage: ".into(),
            self.hytale_mem_usage.to_string().yellow(),
            " GB".into(),
        ])]);

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
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

        for arg in some.iter().skip(1) {
            /* first value is avg then percentage*/
            /* so set first value to string name and second to f64 by parsing the string into f64*/
            let split_test_arg: Vec<&str> = arg.split("=").collect();
            let avg_second = split_test_arg[0];
            let avg_percentage: f64 = split_test_arg[1]
                .parse::<f64>()
                .expect("didnt see a float value");
            /* insert avg_second and avg_percentage as keys into some hashmap and then full hashmap*/
            map_some.insert(avg_second.to_string(), avg_percentage);
        }
        for arg in full.iter().skip(1) {
            /* first value is avgethen percentage*/
            /* so set first value to string name and second to f64 by parsing the string into f64*/
            let split_test_arg: Vec<&str> = arg.split("=").collect();
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
#[derive(Debug, Clone)]
struct HytaleLog {
    first_date: String,
    host_name: String,
    command_name: String,
    time_of_log: String,
    log_type: String,
    info: String,
    who: String,
}
impl HytaleLog {
    fn init_log(one_log_line: Vec<&str>) -> Option<HytaleLog> {
        if one_log_line.len() < 9 {
            // panic!("Invalid log line: {:?}",one_log_line);
            return None;
        }
        // first_date will be month, day, time
        let first_date_slice = &one_log_line[0..3];
        let first_date: String = first_date_slice.join(" ");

        // get host name
        let host_name: String = one_log_line[3].to_string();

        // disclaimer, this assumses that the command name is always start.sh
        // let command_name_slice = &one_log_line[4][0..8];
        // first ai line of code
        let command_name = one_log_line[4].split('[').next().unwrap_or("").to_string();

        let time_of_log_slice = &one_log_line[6..8];
        let time_of_log: String = time_of_log_slice.join(" ");

        let log_type: String = one_log_line[7].to_string();

        let who: String = one_log_line[8].to_string();

        let info_slice = &one_log_line[9..];
        let info: String = info_slice.join(" ");

        Some(HytaleLog {
            first_date,
            host_name,
            command_name,
            time_of_log,
            log_type,
            info,
            who,
        })
    }
}
// this will create a csv file
fn run(struct_avg: AvgResourceUsage) -> Result<(), Box<dyn Error>> {
    let mut file_path = "";
    match struct_avg.system_resource {
        SystemResource::Cpu => file_path = "cpu_usage.csv",
        SystemResource::Mem => file_path = "mem_usage.csv",
    }
    let mut wtr = csv::Writer::from_path(file_path)?;
    // wtr.serialize(struct_avg)?;

    let size_of_map = struct_avg.resource_usage_per_second_map.len();
    match struct_avg.system_resource {
        SystemResource::Cpu => wtr.write_record(["Seconds", "Avg_Cpu_Usage_%"])?,
        SystemResource::Mem => wtr.write_record(["Seconds", "Avg_Mem_Usage_In_GB"])?,
    }
    for i in 1..size_of_map + 1 {
        let temp = i as u32;
        let resource_usage = struct_avg.resource_usage_per_second_map.get(&temp);
        wtr.serialize((i, resource_usage))?;
    }
    wtr.flush()?;
    Ok(())
}
// this will run ratatui screen
fn run_live(interval_ms: i64, sys: &mut System, hytale_pid: u32) -> io::Result<()> {
    ratatui::run(|terminal| App::default().run(terminal, interval_ms, sys, hytale_pid))
}
fn main() {
    // parges user input
    let args = Cli::parse();
    // check if we have a config
    if let Some(res) = check_if_config_dir_exist("adamantite".to_string()){
        if !res 
            && let Some(data_path) = return_config_dir("adamantite".to_string())
            && let Err(err) = create_dir_for_cli(data_path)
            // assuming that we dont have a config directory, we should prompt user for generating a
        // config
        {
            eprint!(
                "Something went wrong with creating config dir for adamantite. Err: {err}"
            )
        }

    }else{
        println!("something went wrong in fetching xdg directories");
        process::exit(1)
    }

    // now we check for simple configuration
    let data_path = return_config_dir("adamantite".to_string()).unwrap();

    let full_path = data_path + "/config";
    let settings = Config::builder()
        // add in `~/.config/adamantite/config.toml`
        .add_source(config::File::with_name(&full_path))
        .build()
        .unwrap();

    let table = settings.get_table("search_method").unwrap();
    let other_table = settings.get_table("server").unwrap();
    // println!(
    // "{:?}",
    //     table
    // );
    let service_method = table.get("method").unwrap();
    let service_name = table.get("service_name").unwrap();
    println!("method: {:?}\n and service_name{:?}",service_method,service_name);
    // allow user to set to search via systemd
    // [search-method]
    // # this systemd is default, hytale is default service name
    // method="systemd"
    // name="hytale"

    let mut sys = System::new();
    sys.refresh_all();
    let num_of_cpus = sys.cpus().len();
    let hytale_pid = find_pid_of_hytale(table);
    let num_of_cpus = num_of_cpus as f32;
    match &args.command {
        Some(Commands::Track {
            system_resource,
            time_seconds,
            output,
        }) => match system_resource {
            SystemResource::Cpu => {
                return_avg_system_cpu_usage_and_hytale_cpu_usage(
                    num_of_cpus,
                    time_seconds,
                    &mut sys,
                    hytale_pid,
                    output,
                );
            }
            SystemResource::Mem => {
                return_avg_system_mem_usage_and_hytale_mem_usage(
                    time_seconds,
                    &mut sys,
                    hytale_pid,
                    output,
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
        Some(Commands::Live { interval_ms }) => {
            if let Err(err) = run_live(*interval_ms as i64, &mut sys, hytale_pid) {
                println!("{}", err);
                process::exit(1);
            }
        }
        None => {
            todo!()
        }
    }
}
fn find_pid_of_hytale(settings: Map<String,Value>) -> u32 {
    // check which settings have been set
    // if settings have been set, us systemd command
    let data_path = return_config_dir("adamantite".to_string()).unwrap();

    let full_path = data_path + "/config";
    let settings = Config::builder()
        // add in `~/.config/adamantite/config.toml`
        .add_source(config::File::with_name(&full_path))
        .build()
        .unwrap();

    let table = settings.get_table("search_method").unwrap();
    let service_method = table.get("method").unwrap();
    let service_name = table.get("service_name").unwrap();
    println!("method: {:?}\n and service_name{:?}",service_method,service_name);
    if let Ok(method) = service_method.clone().into_string() {
        if method == "systemd"{
            let name = service_name.clone().into_string().expect("failed to turn service name into a valid string");
            let mut systemd_cgls_child = Command::new("/bin/systemd-cgls")
                .arg("-u")
                .arg(name)
                .arg("--no-pager")
                .stdout(Stdio::piped())
                .spawn()
                .expect("failed to start systemd");
            systemd_cgls_child.wait().expect("failed to wait on ps");
            let systemd_cgls_out = systemd_cgls_child.stdout.expect("failed to start echo process");
            // let output = systemd_cgls_child.wait_with_output().expect("failed to wain on systemd");
            // let soutput = String::from_utf8_lossy(&output.stdout).to_string();
            // println!("systemd output\n{}",soutput );
    let mut awk_child = Command::new("/bin/awk")
        .arg("END {print $1}")
        .stdin(Stdio::from(systemd_cgls_out))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start awk process");

    awk_child.wait().expect("failed to wait on awk child");

        let output = awk_child
            .wait_with_output()
            .expect("failed to wait for awk");
        let s = String::from_utf8_lossy(&output.stdout).to_string();
            println!("pid: {}, size is {}",s,s.chars().count());
        let new_s = &s[3..];
        println!("pid from slice: {}",new_s);
        }else{
            eprintln!("there is no such service method. Please use `systemd`")
        }
    }else{
        eprintln!("Transforming service_method into a string failed")
    }
    let mut ps_child = Command::new("/bin/ps")
        .arg("aux")
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start ps");
    ps_child.wait().expect("failed to wait on ps");
    let ps_out = ps_child.stdout.expect("failed to start echo process");

    let mut grep_child = Command::new("/bin/grep")
        .arg("java -jar HytaleServer.jar --assets ../Assets.zip --backup --backup-dir backups --backup-frequency 30")
        .stdin(Stdio::from(ps_out))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed te start grep process");

    grep_child.wait().expect("failed to wait on child");

    let grep_output = grep_child.stdout.expect("failed to get grep output");

    let mut head_child = Command::new("/bin/head")
        .arg("-n")
        .arg("1")
        .stdin(Stdio::from(grep_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start the head process");

    head_child.wait().expect("failed to wait on head child");

    let head_output = head_child.stdout.expect("failed to get head output");

    let mut awk_child = Command::new("/bin/awk")
        .arg("{print $2}")
        .stdin(Stdio::from(head_output))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start awk process");

    awk_child.wait().expect("failed to wait on awk child");

    let output = awk_child
        .wait_with_output()
        .expect("failed to wait for awk");
    let s = String::from_utf8_lossy(&output.stdout).to_string();
    let pid_from_s: u32 = s.trim().parse().expect("not a valid number");
    pid_from_s
}
fn get_hytale_logs() -> String {
    let journalctl_child = Command::new("/bin/journalctl")
        .arg("-u")
        // could depend on service name so maybe add config for this
        .arg("hytale")
        .arg("--since")
        .arg("500ms ago")
        .output()
        .expect("failed to start journalctl");
    String::from_utf8_lossy(&journalctl_child.stdout).to_string()
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
    if let Some(process) = s.process(Pid::from_u32(pid))
        && let Some(tasks) = process.tasks()
    {
        for task_pid in tasks {
            if let Some(task) = s.process(*task_pid) {
                let curr_cpu_usage = task.cpu_usage();
                total_cpu_usage += curr_cpu_usage;
            }
        }
    }
    total_cpu_usage
}
fn get_mem_usage_from_pid(pid: u32) -> u64 {
    let s = System::new_all();
    let mut pid_mem_usage: u64 = 0;
    if let Some(process) = s.process(Pid::from_u32(pid)) {
        pid_mem_usage = process.memory();
    }
    pid_mem_usage
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
// gets hytale logs and returns array of each hytale log line in a vec
fn get_test_journalctl_output(journalctl_output: String) -> Result<Vec<HytaleLog>, CustomError> {
    let empty_journalctl_output: String = String::from("-- No entries --\n");
    if journalctl_output == empty_journalctl_output {
        return Err(CustomError::NoJournalCtlOutput);
    }
    let line_of_logs: Vec<&str> = journalctl_output.lines().collect();

    let mut logs_white_space: Vec<Vec<&str>> = Vec::new();

    let mut log_structs: Vec<HytaleLog> = Vec::new();

    for line in line_of_logs {
        logs_white_space.push(line.split_whitespace().collect());
    }

    // only push log that is parsable
    for split_line in logs_white_space {
        if let Some(log) = HytaleLog::init_log(split_line) {
            log_structs.push(log)
        }
    }

    Ok(log_structs)
}
fn return_avg_system_cpu_usage_and_hytale_cpu_usage(
    num_of_cpus: f32,
    time_seconds: &i64,
    sys: &mut System,
    hytale_pid: u32,
    output: &OutputType,
) {
    let start_time = Utc::now().time();
    let mut end_time = Utc::now().time();
    let mut diff = end_time - start_time;

    let mut cpu_avg_resource_usg = AvgResourceUsage::init(SystemResource::Cpu);
    let time_delta = TimeDelta::seconds(*time_seconds);

    let mut last_emit = Instant::now();

    let mut user_selected_time = 1;
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
        total_system_cpu_usage += current_system_cpu_usage / total_available_cpu_percentage;

        let hytale_curr_cpu_usage = get_cpu_usage_from_pid(hytale_pid);

        hytale_total_cpu_usage += hytale_curr_cpu_usage;
        // one second has passed
        if last_emit.elapsed() >= Duration::from_secs(1) {
            time_in_seconds_counter += 1;
            cpu_avg_resource_usg.add_new_entry(
                time_in_seconds_counter,
                ((hytale_total_cpu_usage / counter as f32) / total_available_cpu_percentage)
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
        ((hytale_total_cpu_usage / counter as f32) / total_available_cpu_percentage) * num_of_cpus
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
    // another function to handle output type maybe?
    if *output != OutputType::None
        && let Err(err) = run(cpu_avg_resource_usg)
    {
        println!("{}", err);
        process::exit(1);
    }
}
fn return_avg_system_mem_usage_and_hytale_mem_usage(
    time_seconds: &i64,
    sys: &mut System,
    hytale_pid: u32,
    output: &OutputType,
) {
    let start_time = Utc::now().time();
    let mut end_time = Utc::now().time();
    let mut diff = end_time - start_time;

    let mut mem_avg_resource_usg = AvgResourceUsage::init(SystemResource::Mem);
    let time_delta = TimeDelta::seconds(*time_seconds);
    let mut user_selected_time = 1;
    user_selected_time = *time_seconds;
    let system_total_memory = sys.total_memory();

    let mut last_emit = Instant::now();
    let mut time_in_seconds_counter: u32 = 0;

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
        if last_emit.elapsed() >= Duration::from_secs(1) {
            time_in_seconds_counter += 1;
            let total_hytale_mem_in_gigabytes =
                return_mem_in_gigabytes(total_hytale_mem_usage_in_bytes as f64);
            let average_hytale_mem_usage_gb = total_hytale_mem_in_gigabytes / counter as f64;
            mem_avg_resource_usg
                .add_new_entry(time_in_seconds_counter, average_hytale_mem_usage_gb as f32);
            last_emit += Duration::from_secs(1);
        }
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
    if *output != OutputType::None
        && let Err(err) = run(mem_avg_resource_usg)
    {
        println!("{}", err);
        process::exit(1);
    }
}
fn check_if_config_dir_exist(app_name: String) -> Option<bool> {
    if let Some(proj_dir) = ProjectDirs::from("", "", &app_name) {
        return Some(fs::metadata(proj_dir.config_dir()).is_ok());
    }
    None
}
fn return_config_dir(app_name: String) -> Option<String> {
    if let Some(proj_dir) = ProjectDirs::from("","",&app_name) {
        return Some(proj_dir.config_dir().to_str()?.to_string());
    }
    None
}
fn create_dir_for_cli(dir_path: String) -> std::io::Result<()> {
    fs::create_dir(dir_path)?;
    Ok(())
}
// maybe use this code for testing parsing
// fn get_test_journalctl_output()  -> Vec<HytaleLog> {
//     let path_to_test_output = Path::new("./journalctl_output.txt");
//
//     let mut f = File::open(path_to_test_output).expect("failed to open journalctl_output.txt file");
//     let mut content_test_output = String::new();
//     f.read_to_string(&mut content_test_output)
//         .expect("failed to read the file");
//
//     let line_of_logs: Vec<&str> = content_test_output.lines().collect();
//
//     let mut logs_white_space: Vec<Vec<&str>> = Vec::new();
//
//     let mut log_structs: Vec<HytaleLog> = Vec::new();
//
//     for line in line_of_logs{
//         logs_white_space.push(line.split_whitespace().collect());
//     }
//
//     for split_line in logs_white_space{
//         log_structs.push(HytaleLog::init_log(split_line));
//     }
//
//     return log_structs;
// }
