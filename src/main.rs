#![allow(warnings)]
#![allow(unused)]
#![allow(dead_code)]

use std::thread;
use std::io::{ self, Write };
use std::net::{ IpAddr, SocketAddr, TcpStream };
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{ AtomicU32, Ordering };
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{ Duration, Instant };

const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

enum ScanEvent {
    Open(u16),
    Progress,
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
            println!("{BOLD}Discovered Hosts:{RESET}");
            for (idx, ip) in active_ips.iter().enumerate() {
                println!("[{}] {}", idx + 1, ip);
            }

            println!("[0] Rescan / Change Subnet");
            println!("[99] Exit NetScope");

            let choice = prompt_number_in_range(
                &format!("\nSelect a host (1-{}, or 0/99): ", active_ips.len()),
                0,
                99
            );

            if choice == 0 {
                continue 'app;
            }
            if choice == 99 {
                break 'app;
            }

            let selected_index = choice - 1;
            if selected_index >= active_ips.len() {
                eprintln!("Invalid selection.");
                continue 'target;
            }

            let target_ip = active_ips[selected_index];

            'action: loop {
                println!("\n{BOLD}Target: {target_ip}{RESET}");
                println!("  [1] Ping Latency Check");
                println!("  [2] Scan Ports");
                println!("  [3] Select Another Host");
                println!("  [0] Exit NetScope");

                let action = prompt_number_in_range("Choose action (0-3): ", 0, 3);

                match action {
                    1 => ping_host(target_ip),
                    2 => configure_and_run_port_scan(target_ip),
                    3 => {
                        continue 'target;
                    }
                    0 => {
                        break 'app;
                    }
                    _ => {}
                }
            }
        }
    }
    println!("\n{BOLD}Goodbye!{RESET}");
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
    println!("\n{BOLD}Port Scan Profiles:{RESET}");
    println!("  [1] Quick Scan (Top 20 common ports)");
    println!("  [2] Full Range Scan (1 - 65535)");
    println!("  [3] Custom Range");
    println!("  [0] Back");

    let choice = prompt_number_in_range("Select profile (0-3): ", 0, 3);

    match choice {
        1 => {
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
        2 => port_scan_range(ip, 1, 65535),
        3 => {
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
