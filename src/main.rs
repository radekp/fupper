use std::cmp;
use std::fs;
use std::fs::DirEntry;
use std::io;
use std::io::{BufRead, BufReader};
use std::io::{Error, ErrorKind};
use std::{thread, time};
use std::process::Command;

fn choose_iface() -> Result<DirEntry, Error> {
    let stdin = io::stdin();
    let mut iterator = stdin.lock().lines();
    let entries = fs::read_dir("/sys/class/net")?;
    for entry in entries {
        let dir_entry = entry?;
        println!("use iface {:?} (y/n)", dir_entry.file_name());
        let line = iterator.next().unwrap().unwrap();
        if line == "y" {
            return Ok(dir_entry);
        }
    }
    Err(Error::new(ErrorKind::Other, "no interface selected"))
}

fn read_stat(iface_dir: &DirEntry, file_name: &str) -> Result<u64, Error> {
    let mut path = iface_dir.path();
    path.push("statistics");
    path.push(file_name);

    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(_) => panic!("Unable to read title from {:?}", &path),
    };
    let mut buffer = BufReader::new(file);
    let mut first_line = String::new();
    buffer.read_line(&mut first_line)?;

    Ok(first_line.trim().parse::<u64>().unwrap())
}

fn main() -> Result<(), Error> {
    let iface_dir = choose_iface()?;
    let iface_filename = iface_dir.file_name();
    let mut iface_name_cow = iface_filename.to_string_lossy();
    let iface_name = iface_name_cow.to_mut();

    let time_init = time::Instant::now();
    let rx_ini = read_stat(&iface_dir, "rx_bytes")?;
    let tx_ini = read_stat(&iface_dir, "tx_bytes")?;

    let mut bandwidth = 1024; // 1024kb/s
    let ok_speed = 3733; // 10_000_000_000 / 31 * 24 * 60 * 60;      // 10GB/31days

    loop {
        thread::sleep(time::Duration::from_secs(1));

        let rx = read_stat(&iface_dir, "rx_bytes")?;
        let tx = read_stat(&iface_dir, "tx_bytes")?;
        let speed = (rx - rx_ini + tx - tx_ini) / time_init.elapsed().as_secs();

        println!(
            "{} rx={} tx={} speed={}b/s ok_speed={}b/s bandwidth={}kb/s",
            iface_name,
            rx - rx_ini,
            tx - tx_ini,
            speed,
            ok_speed,
            bandwidth
        );

        bandwidth = if speed > ok_speed {
            cmp::max(1, bandwidth / 2)
        } else if bandwidth >= 1024 * 1024 {
            continue;
        } else {
            bandwidth * 2
        };

        // tc qdisc del root dev eth0
        Command::new("tc")
            .arg("qdisc")
            .arg("del")
            .arg("root")
            .arg("dev")
            .arg(&iface_name)
            .status()
            .expect("failed to execute process");

        // tc qdisc add dev ppp0 root tbf rate 1mbit burst 1024kbit latency 1ms
        Command::new("tc")
            .arg("qdisc")
            .arg("add")
            .arg("dev")
            .arg(&iface_name)
            .arg("root")
            .arg("tbf")
            .arg("rate")
            .arg(format!("{}kbit", bandwidth))
            .arg("burst")
            .arg(format!("{}kbit", bandwidth))
            .arg("latency")
            .arg("1ms")
            .status()
            .expect("failed to execute process");
    }
}
