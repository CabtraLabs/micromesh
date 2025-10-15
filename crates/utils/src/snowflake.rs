use std::time::Duration;

use parking_lot::Mutex;

const ALPHABET57: [u8; 57] = [
    b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', 
    b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'J', b'K', b'L', b'M',
    b'N', b'P', b'Q', b'R', b'S', b'T', b'U', b'V', b'W', b'X', b'Y', b'Z',
    b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'm', 
    b'n', b'o', b'p', b'q', b'r', b's', b't', b'u',b'v', b'w', b'x', b'y', b'z'
];

const ALPHABET33: [u8; 33] = [
    b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', 
    b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', b'k', b'm', 
    b'n', b'o', b'p', b'q', b'r', b's', b't', b'u',b'v', b'w', b'x', b'y', b'z'
];

const WORKER_ID_BITS: i64 = 10;
const SEQUENCE_BITS: i64 = 12;
const TIMESTAMP_BITS: i64 = 41;

// use bit operations to get max number of the item
const MAX_WORKER_ID: i64 = -1 ^ (-1 << WORKER_ID_BITS);

// shift bits
const WORKER_ID_SHIFT: i64 = SEQUENCE_BITS;
const TIMESTAMP_LEFT_SHIFT: i64 = SEQUENCE_BITS + WORKER_ID_BITS;

// masks
const SEQUENCE_MASK:i64 = -1 ^ (-1 << SEQUENCE_BITS);
const EPOCH: i64 = 1_730_203_481_000;


pub struct Snowflake {
    worker_id: i64,
    // Use Mutex to protect sequence and last_timestamp
    inner: Mutex<SnowflakeInner>,
}

struct SnowflakeInner {
    sequence: i64,
    last_timestamp: i64,
}

impl Snowflake {
    pub fn k8s() -> Self { 
        // Read WORKER_ID from environment variables
        let worker_id: i64 = if let Some(v) = crate::vars::get_server_id(){
            v
        } else {
            // If not exists, use the last segment of IP address as worker_id
            let ip = get_ip();
            let ip_split: Vec<&str> = ip.split(".").collect();
            // Use 16bits ip address as worker_id
            (ip_split[2].to_string().parse::<i64>().unwrap() << 8) | (ip_split[3].to_string().parse::<i64>().unwrap())
        };
        Snowflake::new(worker_id)
    }

    pub fn new(worker_id: i64) -> Self {
        let worker_id = worker_id % (MAX_WORKER_ID + 1);
        tracing::info!("xid::id::worker_id:{worker_id}");
        Snowflake {
            worker_id,
            inner: Mutex::new(SnowflakeInner {
                sequence: 0,
                last_timestamp: 0,
            }),
        }
    }

    pub fn next_id(&self) -> i64 {
        // Use mutex to protect the entire generation process
        let mut inner = self.inner.lock();
        
        let mut timestamp = self.get_time();
        
        // Handle clock callback
        if timestamp < inner.last_timestamp {
            // Wait for clock to catch up
            while timestamp < inner.last_timestamp {
                // Release lock to give other threads a chance
                drop(inner);
                std::thread::sleep(Duration::from_millis(1));
                timestamp = self.get_time();
                inner = self.inner.lock();
            }
        }
    
        if timestamp == inner.last_timestamp {
            // Within same millisecond, increment sequence
            inner.sequence = (inner.sequence + 1) & SEQUENCE_MASK;
            if inner.sequence == 0 {
                // Sequence exhausted, wait for next millisecond
                timestamp = self.till_next_millis(inner.last_timestamp);
            }
        } else {
            // New millisecond, reset sequence
            inner.sequence = 0;
        }
    
        inner.last_timestamp = timestamp;
    
        // Assemble ID
        _v(timestamp, TIMESTAMP_BITS, TIMESTAMP_LEFT_SHIFT) |
        _v(self.worker_id, WORKER_ID_BITS, WORKER_ID_SHIFT) | 
        _v(inner.sequence, SEQUENCE_BITS, 0)
    }

    fn till_next_millis(&self, last_timestamp: i64) -> i64 {
        let mut timestamp = self.get_time();
        while timestamp <= last_timestamp {
            std::thread::sleep(Duration::from_micros(100));
            timestamp = self.get_time();
        }
        timestamp
    }

    fn get_time(&self) -> i64 {
        chrono::Utc::now().timestamp_millis() - EPOCH
    }
}

fn pow(x :i64, y :i64) -> i64 {
    if y == 0 {
        1
    } else {
        x * pow(x, y-1)
    }
}

fn _v(val: i64, n: i64, shift: i64) -> i64 {
	(val & (pow(2, n) - 1)) << shift
}

pub fn get_ip() -> String {
    std::env::var("POD_IP").unwrap_or("127.0.0.1".to_owned())
}


lazy_static::lazy_static! {
    pub static ref SNOWFLAKE: Snowflake  = Snowflake::k8s();
}

pub fn generate_id()-> i64 {
    SNOWFLAKE.next_id()
}

pub  fn generate_id_str()-> String {
    to_str(SNOWFLAKE.next_id())
}


pub fn parse_id(s: &str)->i64 {
    // println!("parse_id: {s}");
    let alpha_len = ALPHABET33.len() as i64;
    let mut num = 0i64;

    for byte in s.as_bytes() {
        let opt = ALPHABET33.iter().position(|&c| c == *byte);
        if opt.is_none() {
            return generate_id();
        }
        let index = opt.unwrap() as i64;
        num = num * alpha_len + index;
    }
    num
}

pub fn parse_id_base57(s: &str)->i64 {
    // println!("parse_id_base57: {s}");
    let alpha_len = ALPHABET57.len() as i64;
    let mut num = 0i64;

    for byte in s.as_bytes() {
        let opt = ALPHABET57.iter().position(|&c| c == *byte);
        if opt.is_none() {
            return generate_id();
        }
        let index = opt.unwrap() as i64;
        num = num * alpha_len + index;
    }
    num
}

pub fn to_str(id: i64) -> String {
    let mut num = id;
    let mut bytes = Vec::new();
    let alpha_len = ALPHABET33.len() as i64;
    while num > 0 {
        bytes.push(ALPHABET33[(num % alpha_len) as usize]);
        num /= alpha_len;
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap()
}

pub fn to_str_base57(id: i64) -> String {
    let mut num = id;
    let mut bytes = Vec::new();
    let alpha_len = ALPHABET57.len() as i64;
    while num > 0 {
        bytes.push(ALPHABET57[(num % alpha_len) as usize]);
        num /= alpha_len;
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_snowflake() {
        println!("{} {}", SNOWFLAKE.worker_id , MAX_WORKER_ID);
        for _ in 0.. 100 {
            let id = generate_id();
            let id_str: String = to_str(id);
            println!("{} {} {}", id , id_str , parse_id(&id_str));
            
            std::thread::sleep(Duration::from_micros(10));
        }
    }

    #[test]
    fn test_concurrent_generation() {
        use std::collections::HashSet;
        use std::sync::Arc;
        use std::sync::Mutex;
        
        let ids = Arc::new(Mutex::new(HashSet::new()));
        let threads: Vec<_> = (0..100)
            .map(|_| {
                let ids = ids.clone();
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        let id = generate_id();
                        let mut set = ids.lock().unwrap();
                        assert!(set.insert(id), "Duplicate ID generated: {id}");
                    }
                })
            })
            .collect();
    
        for thread in threads {
            thread.join().unwrap();
        }
    }

    #[test]
    fn test_parse_id() {
        let id = parse_id_base57("3vTErqVS35");
        println!("3vTErqVS35->{id}");
    }
}
