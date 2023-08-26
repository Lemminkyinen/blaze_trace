use std::net::IpAddr;

const MAX_PORT: u16 = 65535;

pub fn generate_ip_range(ip_start: IpAddr, ip_end: IpAddr) -> Vec<IpAddr> {
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

            let mut current_segments = start_segments;

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
        .into_iter()
        .filter(|ip| {
            if let IpAddr::V4(ipv4) = ip {
                let octets = ipv4.octets();
                octets[3] != 0 && octets[3] != 255
            } else {
                true // Keep IPv6 addresses as-is
            }
        })
        .collect()
}

fn increment_ipv6_segments(segments: [u16; 8]) -> [u16; 8] {
    let mut result = segments;
    let mut carry = 1;

    for i in (0..8).rev() {
        let sum = u32::from(segments[i]) + carry;
        result[i] = (sum & 0xFFFF) as u16;
        carry = sum >> 16;
    }

    result
}

pub fn create_ip_list_with_ports(ip_range: Vec<IpAddr>, ports: Vec<u16>) -> Vec<(IpAddr, u16)> {
    ip_range
        .into_iter()
        .flat_map(|ip| {
            ports.iter().cloned().filter_map(move |port| {
                if port <= MAX_PORT {
                    Some((ip, port))
                } else {
                    None
                }
            })
        })
        .collect()
}

pub fn get_exact_chunks<T>(iterable: Vec<T>, chunks_amount: usize) -> Vec<Vec<T>>
where
    T: Clone,
{
    let mut chunks: Vec<Vec<_>> = Vec::new();
    let chunk_size = iterable.len() / chunks_amount;
    let mut remainder = iterable.len() % chunks_amount;
    let mut start = 0;
    for _ in 0..chunks_amount {
        let end = start + chunk_size + usize::from(remainder > 0);
        chunks.push(iterable[start..end].to_vec());
        start = end;
        remainder = remainder.saturating_sub(1);
    }
    chunks
}
