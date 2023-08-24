use crossterm::{
    cursor,
    terminal::{Clear, ClearType},
};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::process;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::{env, net::IpAddr, str::FromStr};
const MAX_PORT: u16 = 65535;
const COMMON_PORTS: [u16; 30] = [
    20,    // FTP Data
    21,    // FTP Control
    22,    // SSH
    23,    // Telnet
    25,    // SMTP
    53,    // DNS
    80,    // HTTP
    110,   // POP3
    115,   // SFTP
    119,   // NNTP
    123,   // NTP
    143,   // IMAP
    161,   // SNMP
    194,   // IRC
    443,   // HTTPS
    445,   // Microsoft-DS (SMB)
    587,   // SMTP Submission
    993,   // IMAPS
    995,   // POP3S
    1433,  // Microsoft SQL Server
    1521,  // Oracle Database
    3306,  // MySQL
    3389,  // RDP
    5432,  // PostgreSQL
    5900,  // VNC
    8080,  // HTTP Alternative
    8443,  // HTTPS Alternative
    9090,  // Another HTTP Alternative
    9200,  // Elasticsearch
    27017, // MongoDB
];

fn clear_previous_lines(num_lines: usize) {
    print!("{}", cursor::MoveUp(num_lines as u16));
    for _ in 0..num_lines {
        print!("{}", Clear(ClearType::CurrentLine));
        print!("{}", cursor::MoveDown(1));
    }
    print!("{}", cursor::MoveUp(num_lines as u16));
}
#[derive(Debug)]
struct Args {
    flag: Option<String>,
    ip_range_from: IpAddr,
    ip_range_to: IpAddr,
    threads: u16,
}

impl Args {
    fn new(args: &[String]) -> Result<Args, &'static str> {
        match args.len() {
            2 if (args[1] == "-h" || args[1] == "--help") => {
                println!(
                    "Usage: -j to select how many threads you want. Default and max = 255\n\
                     -h or --help to show this help message"
                );
                return Err("help");
            }
            3 => {
                if args[1] == "-j" {
                    return Err("Invalid syntax");
                }
                let ip_from_arg = args[1].clone();
                let ip_to_arg = args[2].clone();
                if let (Ok(ip_from), Ok(ip_to)) =
                    (IpAddr::from_str(&ip_from_arg), IpAddr::from_str(&ip_to_arg))
                {
                    return Ok(Args {
                        flag: None,
                        ip_range_from: ip_from,
                        ip_range_to: ip_to,
                        threads: 255,
                    });
                } else {
                    return Err("Not a valid IPADDR; must be IPv4 or IPv6");
                }
            }
            5 => {
                if args[1] == "-j" {
                    let threads = match args[2].parse::<u16>() {
                        Ok(s) => s,
                        Err(_) => return Err("Failed to parse thread number"),
                    };
                    let ip_from = match IpAddr::from_str(&args[3]) {
                        Ok(s) => s,
                        Err(_) => return Err("Not a valid IPADDR; must be IPv4 or IPv6"),
                    };
                    let ip_to = match IpAddr::from_str(&args[4]) {
                        Ok(s) => s,
                        Err(_) => return Err("Not a valid IPADDR; must be IPv4 or IPv6"),
                    };
                    return Ok(Args {
                        threads,
                        flag: Some(args[1].clone()),
                        ip_range_from: ip_from,
                        ip_range_to: ip_to,
                    });
                }
            }
            _ => return Err("Invalid number of arguments"),
        }

        Err("Invalid syntax")
    }
}

fn generate_ip_range(ip_start: IpAddr, ip_end: IpAddr) -> Vec<IpAddr> {
    let mut ip_range = Vec::new();
    match (ip_start, ip_end) {
        (IpAddr::V4(start), IpAddr::V4(end)) => {
            let mut current_ip = u32::from(start);
            while current_ip <= u32::from(end) {
                ip_range.push(IpAddr::V4(current_ip.into()));
                current_ip += 1;
            }
        }
        (IpAddr::V6(start), IpAddr::V6(end)) => {
            let start_segments = start.segments();
            let end_segments = end.segments();

            let mut current_segments = start_segments.clone();

            while current_segments <= end_segments {
                ip_range.push(IpAddr::V6(current_segments.into()));
                current_segments = increment_ipv6_segments(current_segments);
            }
        }
        _ => {
            println!("Mixed IPv4 and IPv6 addresses are not supported");
        }
    }

    ip_range
}

fn increment_ipv6_segments(segments: [u16; 8]) -> [u16; 8] {
    let mut result = segments.clone();
    let mut carry = 1;

    for i in (0..8).rev() {
        let sum = u32::from(segments[i]) + carry;
        result[i] = (sum & 0xFFFF) as u16;
        carry = sum >> 16;
    }

    result
}

fn scan_ports(ip: IpAddr, ports: Vec<u16>, tx: Sender<(IpAddr, u16)>) {
    for port in &ports {
        let socket_addr = if port < &MAX_PORT {
            SocketAddr::new(ip, *port)
        } else {
            println!("Skipped {}:{}, port too big", ip, port);
            continue;
        };
        if let Ok(_) = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(100)) {
            tx.send((ip, *port)).unwrap()
        }
    }
}

fn main() {
    let start_time = Instant::now();
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let arguments = Args::new(&args).unwrap_or_else(|err| {
        if err.contains("help") {
            process::exit(0);
        } else {
            eprintln!("{} problem parsing arguments: {}", program, err);
            process::exit(0);
        }
    });

    let ip_start = arguments.ip_range_from;
    let ip_end = arguments.ip_range_to;
    let ip_range = generate_ip_range(ip_start, ip_end);
    let num_threads = if ip_range.len() < arguments.threads as usize {
        ip_range.len()
    } else if arguments.threads < 255 {
        arguments.threads.into()
    } else {
        255
    };
    let common_ports = COMMON_PORTS.to_vec();
    let chunk_size = ip_range.len() / num_threads as usize;
    let mut remainder = ip_range.len() % num_threads as usize;

    let mut chunks: Vec<Vec<_>> = Vec::new();
    let mut start = 0;

    for _ in 0..num_threads {
        let end = start + chunk_size + if remainder > 0 { 1 } else { 0 };
        chunks.push(ip_range[start..end].to_vec());
        start = end;
        if remainder > 0 {
            remainder -= 1;
        }
    }
    let ip_range_arc = Arc::new(Mutex::new(chunks));
    let (tx, rx) = channel::<(IpAddr, u16)>();
    let common_ports = common_ports.clone();
    let ip_range_arc = Arc::clone(&ip_range_arc);
    let mut threads = 0;
    while let Some(ip_chunk) = ip_range_arc.lock().unwrap().pop() {
        // println!("IP chunk: {:?}", ip_chunk);
        let tx = tx.clone();
        let common_ports = common_ports.clone();
        thread::spawn(move || {
            // println!("IP: {}", ip);
            let tx = tx.clone();
            let common_ports = common_ports.clone();
            for ip in ip_chunk {
                scan_ports(ip, common_ports.clone(), tx.clone());
            }
        });
        threads += 1;
    }
    drop(tx);
    println!("Threads spawned {}\n\n", threads);
    let mut ips: Vec<(IpAddr, u16)> = vec![];
    for (ip, port) in rx {
        clear_previous_lines(ips.len());
        ips.push((ip, port));
        ips.sort_by(|a, b| a.cmp(b));
        let ips_to_print = ips
            .iter()
            .map(|(ip, port)| format!("{}:{} is open!\n", ip, port))
            .collect::<String>();
        print!("{}", ips_to_print);
    }
    let end_time = Instant::now();
    let elapsed_time = end_time - start_time;
    println!(
        "Port scanning completed in {:.2} seconds ({} milliseconds)",
        elapsed_time.as_secs_f64(),
        elapsed_time.as_millis()
    );
}
