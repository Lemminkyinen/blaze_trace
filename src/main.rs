// #![warn(clippy::pedantic, clippy::nursery, clippy::cargo)]
mod utils;
use clap::Parser;
use crossterm::{
    cursor,
    terminal::{Clear, ClearType},
};
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

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
#[derive(Debug, Parser)]
#[command(author, version, about, long_about=None)]
struct Args {
    ip_range_from: IpAddr,
    ip_range_to: IpAddr,

    #[arg(short, long, num_args = 0..500, value_delimiter = ' ')]
    ports: Vec<u16>,

    #[arg(short, long, default_value_t = 200)]
    threads: u16,

    #[arg(short = 'o', long, default_value_t = 500)]
    time_out: u64,
}

async fn scan_chunk(ip_chunk: Vec<(IpAddr, u16)>, time_out: u64, tx: mpsc::Sender<(IpAddr, u16)>) {
    let timeout = Duration::from_millis(time_out);
    for (ip, port) in ip_chunk {
        let socket_addr = format! {"{}:{}",ip , port}
            .parse::<SocketAddr>()
            .expect("Invalid IP address or port");
        if let Ok(_) = tokio::time::timeout(timeout, TcpStream::connect(socket_addr)).await {
            tx.send((ip, port)).await.expect("Channel send failed");
        }
    }
}

#[tokio::main]
async fn main() {
    let start_time = Instant::now();
    let mut arguments: Args = Args::parse();
    let ip_range = utils::generate_ip_range(arguments.ip_range_from, arguments.ip_range_to);
    let mut ports = COMMON_PORTS.to_vec();
    ports.append(&mut arguments.ports);
    let ips_with_ports = utils::create_ip_list_with_ports(ip_range.clone(), ports);
    let num_threads = arguments.threads as usize;
    let chunks = utils::get_exact_chunks(ips_with_ports, num_threads);
    let ip_range_arc = Arc::new(Mutex::new(chunks));
    let ip_range_arc = Arc::clone(&ip_range_arc);
    let (tx, mut rx) = mpsc::channel(num_threads);

    for ip_chunk in ip_range_arc.lock().unwrap().iter() {
        let tx = tx.clone();
        let ip_chunk = ip_chunk.clone();
        tokio::spawn(async move {
            scan_chunk(ip_chunk, arguments.time_out, tx.clone()).await;
        });
    }
    drop(tx);
    println!("Threads spawned {}\n\n", num_threads);
    let mut ips: Vec<(IpAddr, u16)> = vec![];
    while let Some((ip, port)) = rx.recv().await {
        clear_previous_lines(ips.len());
        ips.push((ip, port));
        ips.sort();
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
