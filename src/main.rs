#![allow(warnings)]
#![allow(unused)]
#![allow(dead_code)]

use std::io::{ self, Read, Write };
use std::net::{ IpAddr, SocketAddr, TcpStream };
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{ AtomicU32, Ordering };
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
        let _ = Command::new("stty").arg("raw").arg("-echo").status();
        print!("\x1b[?25l");
        let _ = io::stdout().flush();
        TerminalModeGuard
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        let _ = Command::new("stty").arg("sane").status();
        print!("\x1b[?25h");
        let _ = io::stdout().flush();
    }
}

fn main() {
    println!("{BOLD}=== NetScope: Network Discovery & Tools ==={RESET}");

    'app: loop {
        let subnet_prefix = prompt_subnet();
        println!("\nDiscovering active hosts on {subnet_prefix}.1..=254...");

        let timer = Instant::now();
        let active_ips = discover_hosts(&subnet_prefix);
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

fn prompt_menu<T: AsRef<str>>(title: &str, options: &[T]) -> usize {
    let _guard = TerminalModeGuard::new();
    let mut selected = 0;
    let mut stdin = io::stdin();

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

        let mut buf = [0u8; 1];
        if stdin.read(&mut buf).is_err() {
            break;
        }

        match buf[0] {
            b'\r' | b'\n' => {
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
            b'i' | b'I' => {
                if selected > 0 {
                    selected -= 1;
                }
            }
            b'k' | b'K' => {
                if selected + 1 < options.len() {
                    selected += 1;
                }
            }
            27 => {
                let mut seq = [0u8; 2];
                if stdin.read_exact(&mut seq).is_ok() && seq[0] == b'[' {
                    match seq[1] {
                        b'A' => {
                            if selected > 0 {
                                selected -= 1;
                            }
                        }
                        b'B' => {
                            if selected + 1 < options.len() {
                                selected += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        print!("\x1b[{}A", options.len() + 1);
    }
    selected
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
            scan_port_list(ip, &common_ports);
        }
        1 => port_scan_range(ip, 1, 65535),
        2 => {
            let start = prompt_number_in_range("Enter start port (1-65535): ", 1, 65535) as u16;
            let end = prompt_number_in_range(
                "Enter end port (1-65535): ",
                start as usize,
                65535
            ) as u16;
            port_scan_range(ip, start, end);
        }
        _ => {}
    }
}

fn port_scan_range(ip: IpAddr, start: u16, end: u16) {
    let ports: Vec<u16> = (start..=end).collect();
    scan_port_list(ip, &ports);
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
    let (tx, rx) = mpsc::channel();
    let mut handles = vec![];

    for host_id in 1..=254 {
        let tx = tx.clone();
        let ip_str = format!("{subnet_prefix}.{host_id}");

        let handle = thread::spawn(move || {
            if let Ok(ip) = IpAddr::from_str(&ip_str) {
                if is_host_alive(ip) {
                    let _ = tx.send(ip);
                }
            }
        });
        handles.push(handle);
    }

    drop(tx);

    let mut active = vec![];
    for ip in rx {
        active.push(ip);
    }

    for handle in handles {
        let _ = handle.join();
    }

    active.sort();
    active
}

fn ping_host(ip: IpAddr) {
    println!("\nPinging {ip}...");
    let status = Command::new("ping").arg("-c").arg("4").arg(ip.to_string()).status();

    match status {
        Ok(s) if s.success() => println!("{GREEN}Ping successful.{RESET}"),
        _ => println!("{GRAY}Ping failed or host unreachable.{RESET}"),
    }
}

fn scan_port_list(ip: IpAddr, ports: &[u16]) {
    if ports.is_empty() {
        println!("{GRAY}No ports specified to scan.{RESET}");
        return;
    }

    println!("\nScanning {} port(s) on {ip} with concurrent workers...", ports.len());
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
            let service = get_service_name(port);
            println!("  Port {GREEN}{port:>5}/tcp{RESET} OPEN ({service})");
        }
    }

    open_ports.sort();
    println!(
        "\nScan complete: Found {} open port(s) in {:.2?}\n",
        open_ports.len(),
        timer.elapsed()
    );
}
