use std::net::{IpAddr, SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

fn main() {
    println!("=== NetScope: IP & Port Scanner ===");

    let targetIpStr = "127.0.0.1";
    let portsToScan = [21, 22, 80, 443, 3000, 8080];

    let ip = match IpAddr::from_str(targetIpStr) {
        Ok(parsedIp) => parsedIp,
        Err(_) => {
            eprintln!("Error: Invalid IP address string!");
            return;
        }
    };

    println!("Scanning target IP: {ip}\n");

    for port in portsToScan {
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