use std::env;
use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::{Duration, Instant};

const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[90m";
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 {
        eprintln!("Usage: {} <ip_address> <start_port> <end_port>", args[0]);
        return;
    }

    let ip = match IpAddr::from_str(&args[1]) {
        Ok(parsed) => parsed,
        Err(_) => {
            eprintln!("Error: Invalid IP address '{}'", args[1]);
            return;
        }
    };

    let start_port: u16 = match args[2].parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Error: Invalid start port '{}'", args[2]);
            return;
        }
    };

    let end_port: u16 = match args[3].parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Error: Invalid end port '{}'", args[3]);
            return;
        }
    };

    let total_ports = (end_port - start_port + 1) as usize;
    let mut open_ports: Vec<u16> = Vec::new();

    println!("{BOLD}=== NetScope Scanner ==={RESET}");
    println!("Target IP : {BOLD}{ip}{RESET}");
    println!("Range     : {start_port}..={end_port} ({total_ports} ports)\n");

    let timer = Instant::now();

    for (index, port) in (start_port..=end_port).enumerate() {
        let current_count = index + 1;

        print!(
            "\r{GRAY}[Scanning {current_count}/{total_ports}] Checking port {port}...{RESET}\x1b[K"
        );
        io::stdout().flush().unwrap();

        if scan_port(ip, port) {
            open_ports.push(port);
            println!("\r{GREEN}[+] Port {:<5} : OPEN{RESET}\x1b[K", port);
        }
    }

    let elapsed = timer.elapsed();

    print!("\r\x1b[K");
    println!("\n{BOLD}=== Scan Summary ==={RESET}");
    println!("Time taken : {:.2?}", elapsed);
    println!("Open ports : {GREEN}{}{RESET}", open_ports.len());

    if !open_ports.is_empty() {
        print!("Found      : ");
        for p in &open_ports {
            print!("{GREEN}{p}{RESET} ");
        }
        println!();
    }
}

fn scan_port(ip: IpAddr, port: u16) -> bool {
    let address = SocketAddr::new(ip, port);
    let timeout = Duration::from_millis(200);

    TcpStream::connect_timeout(&address, timeout).is_ok()
}