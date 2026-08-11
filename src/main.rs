#![allow(warnings)]
#![allow(unused)]
#![allow(dead_code)]

use crossterm::{
    cursor,
    event::{ self, Event, KeyCode, KeyModifiers },
    terminal::{ disable_raw_mode, enable_raw_mode },
    ExecutableCommand,
};
use std::io::{ self, Read, Write };
use std::net::{ IpAddr, SocketAddr, TcpStream };
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{ AtomicBool, Ordering };
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{ Duration, Instant };

const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

enum ScanEvent {
    Open(u16),
    Progress,
}

struct TerminalModeGuard;

impl TerminalModeGuard {
    fn new() -> Self {
        let _ = enable_raw_mode();
        let _ = io::stdout().execute(cursor::Hide);
        TerminalModeGuard
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = io::stdout().execute(cursor::Show);
        let _ = disable_raw_mode();
    }
}

fn main() {
    println!("{BOLD}=== NetScope: Network Discovery & Tools ==={RESET}");

    'app: loop {
        let subnet_prefix = prompt_subnet();
        println!();

        let timer = Instant::now();
        let active_ips = run_with_spinner(
            &format!("Discovering active hosts on {subnet_prefix}.1..=254"),
            || discover_hosts(&subnet_prefix)
        );

        println!("Found {} active host(s) in {:.2?}\n", active_ips.len(), timer.elapsed());

        if active_ips.is_empty() {
            println!("{GRAY}No active hosts detected on this subnet.{RESET}");
            if !prompt_yes_no("Would you like to try another subnet?") {
                break 'app;
            }
            continue 'app;
        }

        'target: loop {
            let mut host_options: Vec<String> = active_ips
                .iter()
                .map(|ip| ip.to_string())
                .collect();
            host_options.push("Rescan / Change Subnet".to_string());
            host_options.push("Exit NetScope".to_string());

            let selection = prompt_menu("Discovered Hosts", &host_options);

            if selection == active_ips.len() {
                continue 'app;
            }
            if selection == active_ips.len() + 1 {
                break 'app;
            }

            let target_ip = active_ips[selection];

            'action: loop {
                let action_options = vec![
                    "Ping Latency Check",
                    "Scan Ports",
                    "Select Another Host",
                    "Exit NetScope"
                ];

                let action_title = format!("Target: {target_ip}");
                let action = prompt_menu(&action_title, &action_options);

                match action {
                    0 => ping_host(target_ip),
                    1 => configure_and_run_port_scan(target_ip),
                    2 => {
                        continue 'target;
                    }
                    3 => {
                        break 'app;
                    }
                    _ => {}
                }
            }
        }
    }
    println!("\n{BOLD}Goodbye!{RESET}");
}

fn run_with_spinner<F, T>(message: &str, f: F) -> T where F: FnOnce() -> T {
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = done.clone();
    let msg = message.to_string();

    let spinner_handle = thread::spawn(move || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0;
        print!("\x1b[?25l");
        while !done_clone.load(Ordering::Relaxed) {
            print!("\r{GRAY}{}{RESET} {}...", frames[idx], msg);
            let _ = io::stdout().flush();
            idx = (idx + 1) % frames.len();
            thread::sleep(Duration::from_millis(80));
        }
        print!("\r\x1b[K");
        let _ = io::stdout().flush();
    });

    let result = f();
    done.store(true, Ordering::Relaxed);
    let _ = spinner_handle.join();
    result
}

fn prompt_menu<T: AsRef<str>>(title: &str, options: &[T]) -> usize {
    let _guard = TerminalModeGuard::new();
    let mut selected = 0;

    loop {
        print!("\r{BOLD}{title}:{RESET}\r\n");
        for (idx, option) in options.iter().enumerate() {
            let label = option.as_ref();
            if idx == selected {
                print!("\r  {GREEN}❯ {label}{RESET}\x1b[K\r\n");
            } else {
                print!("\r    {label}\x1b[K\r\n");
            }
        }
        let _ = io::stdout().flush();

        if let Ok(Event::Key(key_event)) = event::read() {
            match key_event.code {
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    drop(_guard);
                    std::process::exit(0);
                }
                KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                    if selected + 1 < options.len() {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    print!("\x1b[{}A", options.len() + 1);
                    print!(
                        "\r\x1b[K{BOLD}{title}:{RESET} {GREEN}{}{RESET}\r\n",
                        options[selected].as_ref()
                    );
                    for _ in 0..options.len() {
                        print!("\r\x1b[K\r\n");
                    }
                    print!("\x1b[{}A", options.len());
                    let _ = io::stdout().flush();
                    return selected;
                }
                _ => {}
            }
        }
        print!("\x1b[{}A", options.len() + 1);
    }
}

fn prompt_subnet() -> String {
    print!("Enter subnet prefix (default: 192.168.1): ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let subnet = input.trim();
    if subnet.is_empty() {
        "192.168.1".to_string()
    } else {
        subnet.to_string()
    }
}

fn prompt_number_in_range(prompt_msg: &str, min: usize, max: usize) -> usize {
    loop {
        print!("{prompt_msg}");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            if let Ok(num) = input.trim().parse::<usize>() {
                if num >= min && num <= max {
                    return num;
                }
            }
        }
        println!("{GRAY}Invalid option. Enter a number between {min} and {max}.{RESET}");
    }
}

fn prompt_yes_no(prompt_msg: &str) -> bool {
    loop {
        print!("{prompt_msg} (y/n): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => {
                    return true;
                }
                "n" | "no" => {
                    return false;
                }
                _ => {}
            }
            println!("{GRAY}Please answer with 'y' or 'n'.{RESET}");
        }
    }
}

fn get_service_name(port: u16) -> &'static str {
    match port {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        110 => "POP3",
        135 => "RPC",
        139 => "NetBIOS",
        143 => "IMAP",
        443 => "HTTPS",
        445 => "SMB",
        1433 => "MSSQL",
        3306 => "MySQL",
        3389 => "RDP",
        5432 => "PostgreSQL",
        6379 => "Redis",
        8080 => "HTTP-Proxy",
        8443 => "HTTPS-Alt",
        _ => "Unknown",
    }
}

fn configure_and_run_port_scan(ip: IpAddr) {
    let scan_profiles = vec![
        "Quick Scan (Top 20 common ports)",
        "Full Range Scan (1 - 65535)",
        "Custom Range",
        "Back"
    ];

    let choice = prompt_menu("Port Scan Profiles", &scan_profiles);

    match choice {
        0 => {
            let common_ports = vec![
                21,
                22,
                23,
                25,
                53,
                80,
                110,
                135,
                139,
                143,
                443,
                445,
                1433,
                3306,
                3389,
                5432,
                6379,
                8080,
                8443
            ];
            let res = run_with_spinner("Scanning top ports", || {
                scan_port_list(ip, &common_ports)
            });
            print_scan_results(ip, res);
        }
        1 => {
            let res = run_with_spinner("Scanning all 65,535 ports", ||
                port_scan_range(ip, 1, 65535)
            );
            print_scan_results(ip, res);
        }
        2 => {
            let start = prompt_number_in_range("Enter start port (1-65535): ", 1, 65535) as u16;
            let end = prompt_number_in_range(
                "Enter end port (1-65535): ",
                start as usize,
                65535
            ) as u16;
            let res = run_with_spinner(&format!("Scanning ports {start}..={end}"), || {
                port_scan_range(ip, start, end)
            });
            print_scan_results(ip, res);
        }
        _ => {}
    }
}

fn port_scan_range(ip: IpAddr, start: u16, end: u16) -> (Vec<u16>, Duration) {
    let ports: Vec<u16> = (start..=end).collect();
    scan_port_list(ip, &ports)
}

fn is_host_alive(ip: IpAddr) -> bool {
    let socket = SocketAddr::new(ip, 80);
    if TcpStream::connect_timeout(&socket, Duration::from_millis(150)).is_ok() {
        return true;
    }

    let output = Command::new("ping")
        .arg("-c")
        .arg("1")
        .arg("-W")
        .arg("1")
        .arg(ip.to_string())
        .output();

    if let Ok(out) = output {
        out.status.success()
    } else {
        false
    }
}

fn discover_hosts(subnet_prefix: &str) -> Vec<IpAddr> {
    let mut handles = vec![];
    let discovery_ports = [80, 443, 22, 445, 139, 53, 8080, 5353, 62078];

    for i in 1..=254 {
        let ip_str = format!("{subnet_prefix}.{i}");
        let ip: IpAddr = match ip_str.parse() {
            Ok(ip) => ip,
            Err(_) => {
                continue;
            }
        };

        let handle = thread::spawn(move || {
            for &port in &discovery_ports {
                let socket_addr = SocketAddr::new(ip, port);
                match TcpStream::connect_timeout(&socket_addr, Duration::from_millis(200)) {
                    Ok(_) => {
                        return Some(ip);
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                        return Some(ip);
                    }
                    _ => {}
                }
            }
            None
        });
        handles.push(handle);
    }

    let mut active = Vec::new();
    for handle in handles {
        if let Ok(Some(ip)) = handle.join() {
            active.push(ip);
        }
    }
    active.sort();
    active
}

fn ping_host(ip: IpAddr) {
    println!("\nPinging {ip}...");
    let status = Command::new("ping").arg("-c").arg("4").arg(ip.to_string()).status();

    match status {
        Ok(s) if s.success() => println!("{GREEN}Ping successful.{RESET}\n"),
        _ => println!("{GRAY}Ping failed or host unreachable.{RESET}\n"),
    }
}

fn scan_port_list(ip: IpAddr, ports: &[u16]) -> (Vec<u16>, Duration) {
    if ports.is_empty() {
        return (vec![], Duration::from_secs(0));
    }

    let timer = Instant::now();
    let (tx, rx) = mpsc::channel();
    let total_ports = ports.len();

    let worker_count = (250).min(total_ports).max(1);
    let chunk_size = (total_ports + worker_count - 1) / worker_count;

    thread::scope(|s| {
        for chunk in ports.chunks(chunk_size) {
            let tx = tx.clone();
            s.spawn(move || {
                for &port in chunk {
                    let socket = SocketAddr::new(ip, port);
                    if TcpStream::connect_timeout(&socket, Duration::from_millis(250)).is_ok() {
                        let _ = tx.send(ScanEvent::Open(port));
                    }
                }
            });
        }
    });

    drop(tx);

    let mut open_ports = Vec::new();
    for event in rx {
        if let ScanEvent::Open(port) = event {
            open_ports.push(port);
        }
    }

    open_ports.sort();
    (open_ports, timer.elapsed())
}

fn print_scan_results(ip: IpAddr, (open_ports, elapsed): (Vec<u16>, Duration)) {
    if open_ports.is_empty() {
        println!("{GRAY}No open ports found on {ip} in {:.2?}.{RESET}\n", elapsed);
        return;
    }

    println!("\nOpen ports on {ip}:");
    for &port in &open_ports {
        // <-- Added `&` before open_ports
        let service = get_service_name(port);
        println!("  Port {GREEN}{port:>5}/tcp{RESET} OPEN ({service})");
    }
    println!("\nScan complete: Found {} open port(s) in {:.2?}\n", open_ports.len(), elapsed);
}
