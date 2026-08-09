use std::env;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 {
        eprintln!("Usage: {} <ip> <start port> <end port>", args[0]);
        eprintln!("Example: {} 127.0.0.1 20 100", args[0]);
        return;
    }

    let ip = match IpAddr::from_str(&args[1]) {
        Ok(parsedIp) => parsedIp,
        Err(_) => {
            eprintln!("Error: Invalid IP address string!");
            return;
        }
    };

    let startPort = match args[2].parse() {
        Ok(port) => port,
        Err(_) => {
            eprintln!("Error: Invalid start port '{}'", args[2]);
            return;
        }
    };

    let endPort = match args[3].parse() {
        Ok(port) => port,
        Err(_) => {
            eprintln!("Error: Invalid end port '{}'", args[3]);
            return;
        }
    };

    if startPort > endPort {
        eprintln!("Error: Start port cannot be greater than end port");
    }

    println!("=== NetScope: IP & Port Scanner ===");
    println!("Scanning target IP: {ip}\n");

    for port in startPort..=endPort {
        scanPort(ip, port);
    }
}

fn scanPort(ip: IpAddr, port: u16) {
    let address = SocketAddr::new(ip, port);
    let timeout = Duration::from_millis(500);

    match TcpStream::connect_timeout(&address, timeout) {
        Ok(_) => println!("[+] Port {:<5} : OPEN", port),
        Err(_) => println!("[-] Port {:<5} : Closed / Filtered", port)
    }
}