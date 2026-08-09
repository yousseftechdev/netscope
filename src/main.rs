#![allow(warnings)]
#![allow(unused)]
#![allow(dead_code)]

use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::process::Command;
use std::str::FromStr;
use std::sync::mpsc;
use std::{eprint, print, println, thread};
use std::time::{Duration, Instant};

const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

fn main() {
    println!("{BOLD}=== NetScope: Network Discovery & Tools ==={RESET}");

    println!("Enter subnet prefix (default: 192.168.1): ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let subnet = input.trim();
    let subnet_prefix = if subnet.is_empty() {
        "192.168.1"
    } else {
        subnet
    };

    println!("\nDiscovering active hosts on {subnet_prefix}.1..=254...");

    let timer = Instant::now();

    let active_ips= discover_hosts(subnet_prefix);
    println!("Found {} active host(s) in {:.2?}\n", active_ips.len(), timer.elapsed());

    if active_ips.is_empty() {
        println!("No active hosts detectd on this subnet.");
        return;
    }

    for (idx, ip) in active_ips.iter().enumerate() {
        println!("[{}] {}", idx + 1, ip);
    }

    print!("\nSelect a host (1-{}): ", active_ips.len());
    io::stdout().flush().unwrap();

    let selected_index = read_user_number() - 1;
    if selected_index >= active_ips.len() {
        eprintln!("Invalid selection.");
        return;
    }

    let target_ip = active_ips[selected_index];

    println!("\nSelected Target: {target_ip}");
    println!("[1] Ping Latency Check");
    println!("[2] Scan Common Ports");
    print!("Choose action (1 or 2): ");
    io::stdout().flush().unwrap();

        match read_user_number() {
        1 => ping_host(target_ip),
        2 => {
            print!("Enter start port (1-65535): ");
            io::stdout().flush().unwrap();
            let start_port = read_user_port();

            print!("Enter end port (1-65535): ");
            io::stdout().flush().unwrap();
            let end_port = read_user_port();

            if start_port > end_port || start_port == 0 {
                eprintln!("Invalid port range!");
            } else {
                port_scan_host(target_ip, start_port, end_port);
            }
        },
        _ => eprintln!("Invalid choice."),
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

    for handle in handles {
        let _ = handle.join();
    }

    let mut active: Vec<IpAddr> = rx.into_iter().collect();
    active.sort();
    active
}

fn is_host_alive(ip: IpAddr) -> bool {
    let common_ports = [80, 443, 22, 445, 135, 8080, 53];
    let timeout = Duration::from_millis(150);

    for port in common_ports {
        let address = SocketAddr::new(ip, port);
        if TcpStream::connect_timeout(&address, timeout).is_ok() {
            return true;
        }
    }
    false
}

fn ping_host(ip: IpAddr) {
    println!("\n--- Pinging {ip} ---");

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("ping");
        c.args(&["-n", "4", &ip.to_string()]);
        c
    } else {
        let mut c = Command::new("ping");
        c.args(&["-c", "4", &ip.to_string()]);
        c
    };

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{GREEN}{stdout}{RESET}");
        }
        Err(err) => eprintln!("Failed to execute system ping: {err}")
    }
}

fn port_scan_host(ip: IpAddr, start_port: u16, end_port: u16) {
    let total_ports = end_port - start_port + 1;

    println!("\n--- Scanning {total_ports} ports on {ip} ---");

    for port in start_port..=end_port {
        let address = SocketAddr::new(ip, port);
        let timeout = Duration::from_millis(300);

        print!("\r\x1b[K{GRAY}Scanned [ {port}/{total_ports} ]...{RESET}");
        io::stdout().flush().unwrap();

        if TcpStream::connect_timeout(&address, timeout).is_ok() {
            println!("\r\x1b[K{GREEN}[+] Port {:<5} : OPEN{RESET}", port);
        }
    }
    println!("\r\x1b[K{GREEN}Finished scanning!{RESET}")
}

fn read_user_number() -> usize {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().parse::<usize>().unwrap_or(0)
}

fn read_user_port() -> u16 {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().parse::<u16>().unwrap_or(0)
}