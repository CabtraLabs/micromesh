#![allow(dead_code)]
#![allow(unused_variables)]

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, process};
use std::str;
#[cfg(target_os = "macos")]
use sysctl::{Sysctl, SysctlError};

use crc32fast::Hasher;
use rand::RngCore;

#[derive(Debug)]
pub struct Generator {
    counter: AtomicU32,
    machine_id: [u8; 3],
    pid: [u8; 2],
}

pub fn get_generator() -> &'static Generator {
    static INSTANCE: OnceCell<Generator> = OnceCell::new();

    INSTANCE.get_or_init(|| Generator {
        counter: AtomicU32::new(init_random()),
        machine_id: get_machine_id(),
        pid: get_pid().to_be_bytes(),
    })
}

impl Generator {
    pub fn new_id(&self) -> Id {
        self.with_time(&SystemTime::now())
    }

    fn with_time(&self, time: &SystemTime) -> Id {
        // Panic if the time is before the epoch.
        let unix_ts = time
            .duration_since(UNIX_EPOCH)
            .expect("Clock may have gone backwards");
        #[allow(clippy::cast_possible_truncation)]
        self.generate(unix_ts.as_secs() as u32)
    }

    fn generate(&self, unix_ts: u32) -> Id {
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);

        let mut raw = [0_u8; RAW_LEN];
        // 4 bytes of Timestamp (big endian)
        raw[0..=3].copy_from_slice(&unix_ts.to_be_bytes());
        // 3 bytes of Machine ID
        raw[4..=6].copy_from_slice(&self.machine_id);
        // 2 bytes of PID
        raw[7..=8].copy_from_slice(&self.pid);
        // 3 bytes of increment counter (big endian)
        raw[9..].copy_from_slice(&counter.to_be_bytes()[1..]);

        Id(raw)
    }
}

// https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/id.go#L136
fn init_random() -> u32 {
    let mut bs = [0_u8; 3];
    rand::rng().fill_bytes(&mut bs);
    u32::from_be_bytes([0, bs[0], bs[1], bs[2]])
}

pub(crate) const RAW_LEN: usize = 12;
const ENCODED_LEN: usize = 20;
const ENC: &[u8] = "0123456789abcdefghijklmnopqrstuv".as_bytes();

/// An ID.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Id(pub [u8; RAW_LEN]);

impl Default for Id {
    fn default() -> Self {
        new()
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
impl <'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
    
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Invalid length: expected 20 characters, got {0}")]
    InvalidLength(usize),
    #[error("Invalid character '{0}' at position {1}")]
    InvalidCharacter(char, usize),
}

impl std::str::FromStr for Id {
    type Err = DecodeError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != ENCODED_LEN {
            return Err(DecodeError::InvalidLength(value.len()));
        }

        let mut raw = [0_u8; RAW_LEN];
        let bytes = value.as_bytes();

        // Decode base32 encoded string
        raw[0] = (decode_char(bytes[0])? << 3) | (decode_char(bytes[1])? >> 2);
        raw[1] = (decode_char(bytes[1])? << 6) | (decode_char(bytes[2])? << 1) | (decode_char(bytes[3])? >> 4);
        raw[2] = (decode_char(bytes[3])? << 4) | (decode_char(bytes[4])? >> 1);
        raw[3] = (decode_char(bytes[4])? << 7) | (decode_char(bytes[5])? << 2) | (decode_char(bytes[6])? >> 3);
        raw[4] = (decode_char(bytes[6])? << 5) | decode_char(bytes[7])?;
        raw[5] = (decode_char(bytes[8])? << 3) | (decode_char(bytes[9])? >> 2);
        raw[6] = (decode_char(bytes[9])? << 6) | (decode_char(bytes[10])? << 1) | (decode_char(bytes[11])? >> 4);
        raw[7] = (decode_char(bytes[11])? << 4) | (decode_char(bytes[12])? >> 1);
        raw[8] = (decode_char(bytes[12])? << 7) | (decode_char(bytes[13])? << 2) | (decode_char(bytes[14])? >> 3);
        raw[9] = (decode_char(bytes[14])? << 5) | decode_char(bytes[15])?;
        raw[10] = (decode_char(bytes[16])? << 3) | (decode_char(bytes[17])? >> 2);
        raw[11] = (decode_char(bytes[17])? << 6) | (decode_char(bytes[18])? << 1) | (decode_char(bytes[19])? >> 4);

        Ok(Id(raw))
    }
}

    // Helper function: decode single character
fn decode_char(c: u8) -> Result<u8, DecodeError> {
    let pos = ENC.iter().position(|&x| x == c);
    match pos {
        Some(idx) => Ok(idx as u8),
        None => Err(DecodeError::InvalidCharacter(c as char, 0)),
    }
}

impl Id {
    /// The binary representation of the id.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; RAW_LEN] {
        let Self(raw) = self;
        raw
    }

    /// Extract the 3-byte machine id.
    #[must_use]
    pub fn machine(&self) -> [u8; 3] {
        let raw = self.as_bytes();
        [raw[4], raw[5], raw[6]]
    }

    /// Extract the process id.
    #[must_use]
    pub fn pid(&self) -> u16 {
        let raw = self.as_bytes();
        u16::from_be_bytes([raw[7], raw[8]])
    }

    /// Extract the timestamp.
    #[must_use]
    pub fn time(&self) -> SystemTime {
        let raw = self.as_bytes();
        let unix_ts = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
        UNIX_EPOCH + Duration::from_secs(u64::from(unix_ts))
    }

    /// Extract the incrementing counter.
    #[must_use]
    pub fn counter(&self) -> u32 {
        // Counter is stored as big-endian 3-byte value
        let raw = self.as_bytes();
        u32::from_be_bytes([0, raw[9], raw[10], raw[11]])
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(raw) = self;
        let mut bs = [0_u8; ENCODED_LEN];
        bs[19] = ENC[((raw[11] << 4) & 31) as usize];
        bs[18] = ENC[((raw[11] >> 1) & 31) as usize];
        bs[17] = ENC[(((raw[11] >> 6) | (raw[10] << 2)) & 31) as usize];
        bs[16] = ENC[(raw[10] >> 3) as usize];
        bs[15] = ENC[(raw[9] & 31) as usize];
        bs[14] = ENC[(((raw[9] >> 5) | (raw[8] << 3)) & 31) as usize];
        bs[13] = ENC[((raw[8] >> 2) & 31) as usize];
        bs[12] = ENC[(((raw[8] >> 7) | (raw[7] << 1)) & 31) as usize];
        bs[11] = ENC[(((raw[7] >> 4) | (raw[6] << 4)) & 31) as usize];
        bs[10] = ENC[((raw[6] >> 1) & 31) as usize];
        bs[9] = ENC[(((raw[6] >> 6) | (raw[5] << 2)) & 31) as usize];
        bs[8] = ENC[(raw[5] >> 3) as usize];
        bs[7] = ENC[(raw[4] & 31) as usize];
        bs[6] = ENC[(((raw[4] >> 5) | (raw[3] << 3)) & 31) as usize];
        bs[5] = ENC[((raw[3] >> 2) & 31) as usize];
        bs[4] = ENC[(((raw[3] >> 7) | (raw[2] << 1)) & 31) as usize];
        bs[3] = ENC[(((raw[2] >> 4) | (raw[1] << 4)) & 31) as usize];
        bs[2] = ENC[((raw[1] >> 1) & 31) as usize];
        bs[1] = ENC[(((raw[1] >> 6) | (raw[0] << 2)) & 31) as usize];
        bs[0] = ENC[(raw[0] >> 3) as usize];
        write!(f, "{}", str::from_utf8(&bs).unwrap())
    }
}

/* 
impl ToString for Id {
    // https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/id.go#L208
    /// Returns the string representation of the id.
    fn to_string(&self) -> String {
        let Self(raw) = self;
        let mut bs = [0_u8; ENCODED_LEN];
        bs[19] = ENC[((raw[11] << 4) & 31) as usize];
        bs[18] = ENC[((raw[11] >> 1) & 31) as usize];
        bs[17] = ENC[(((raw[11] >> 6) | (raw[10] << 2)) & 31) as usize];
        bs[16] = ENC[(raw[10] >> 3) as usize];
        bs[15] = ENC[(raw[9] & 31) as usize];
        bs[14] = ENC[(((raw[9] >> 5) | (raw[8] << 3)) & 31) as usize];
        bs[13] = ENC[((raw[8] >> 2) & 31) as usize];
        bs[12] = ENC[(((raw[8] >> 7) | (raw[7] << 1)) & 31) as usize];
        bs[11] = ENC[(((raw[7] >> 4) | (raw[6] << 4)) & 31) as usize];
        bs[10] = ENC[((raw[6] >> 1) & 31) as usize];
        bs[9] = ENC[(((raw[6] >> 6) | (raw[5] << 2)) & 31) as usize];
        bs[8] = ENC[(raw[5] >> 3) as usize];
        bs[7] = ENC[(raw[4] & 31) as usize];
        bs[6] = ENC[(((raw[4] >> 5) | (raw[3] << 3)) & 31) as usize];
        bs[5] = ENC[((raw[3] >> 2) & 31) as usize];
        bs[4] = ENC[(((raw[3] >> 7) | (raw[2] << 1)) & 31) as usize];
        bs[3] = ENC[(((raw[2] >> 4) | (raw[1] << 4)) & 31) as usize];
        bs[2] = ENC[((raw[1] >> 1) & 31) as usize];
        bs[1] = ENC[(((raw[1] >> 6) | (raw[0] << 2)) & 31) as usize];
        bs[0] = ENC[(raw[0] >> 3) as usize];
        str::from_utf8(&bs).unwrap().to_string()
    }
}
*/

// 2 bytes of PID
// https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/id.go#L159
#[allow(clippy::cast_possible_truncation)]
pub fn get_pid() -> u16 {
    // https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/id.go#L105
    // > If /proc/self/cpuset exists and is not /, we can assume that we are in a
    // > form of container and use the content of cpuset xor-ed with the PID in
    // > order get a reasonable machine global unique PID.
    let pid = match fs::read("/proc/self/cpuset") {
        Ok(buff) if buff.len() > 1 => process::id() ^ crc32(&buff),
        _ => process::id(),
    };

    pid as u16
}

fn crc32(buff: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(buff);
    hasher.finalize()
}

/// Generate a new globally unique id.
#[must_use]
pub fn new() -> Id {
    get_generator().new_id()
}

// https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/id.go#L117
pub fn get_machine_id() -> [u8; 3] {
    let id = match machine_id().unwrap_or_default() {
        x if !x.is_empty() => x,
        _ => hostname::get()
            .map(|s| s.into_string().unwrap_or_default())
            .unwrap_or_default(),
    };

    let mut bytes = [0_u8; 3];
    if id.is_empty() {
        // Fallback to random bytes
        rand::rng().fill_bytes(&mut bytes);
    } else {
        bytes.copy_from_slice(&md5::compute(id)[0..3]);
    }
    bytes
}

// https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/hostid_linux.go
// Not checking "/sys/class/dmi/id/product_uuid" because normal users can't read it.
#[cfg(target_os = "linux")]
fn machine_id() -> std::io::Result<String> {
    // Get machine-id and remove the trailing new line.
    fs::read_to_string("/var/lib/dbus/machine-id")
        .or_else(|_| fs::read_to_string("/etc/machine-id"))
        .map(|s| s.trim_end().to_string())
}

// https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/hostid_darwin.go
#[cfg(target_os = "macos")]
fn machine_id() -> Result<String, SysctlError> {
    sysctl::Ctl::new("kern.uuid")?
        .value()
        .map(|v| v.to_string())
}

// https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/hostid_windows.go
#[cfg(target_os = "windows")]
fn machine_id() -> std::io::Result<String> {
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    let guid: String = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Cryptography")?
        .get_value("MachineGuid")?;
    Ok(guid)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn machine_id() -> std::io::Result<String> {
    // Fallback to hostname or a random value
    Ok("".to_string())
}

#[cfg(test)]
mod tests {
    // https://github.com/rs/xid/blob/efa678f304ab65d6d57eedcb086798381ae22206/id_test.go#L64
    #[test]
    fn test_new() {
        let mut ids = Vec::new();
        for _ in 0..10 {
            ids.push(super::new());
        }

        for i in 1..10 {
            // Test for uniqueness among all other 9 generated ids
            for j in 0..10 {
                if i != j {
                    assert_ne!(ids[i], ids[j]);
                }
            }

            let id = &ids[i];
            let prev_id = &ids[i - 1];
            // Check that timestamp was incremented and is within 5 seconds of the previous one
            // Panics if it went backwards.
            let secs = id.time().duration_since(prev_id.time()).unwrap().as_secs();
            assert!(secs <= 5);
            // Check that machine ids are the same
            assert_eq!(id.machine(), prev_id.machine());
            // Check that pids are the same
            assert_eq!(id.pid(), prev_id.pid());
            // Test for proper increment
            assert_eq!(id.counter() - prev_id.counter(), 1);
            let s = id.to_string();
            println!("{s}");
        }
    }

    #[test]
    fn test_from_str() {
        // test parse id from string
        let id = super::new();
        let id_str = id.to_string();
        let parsed_id = id_str.parse::<super::Id>().unwrap();
        assert_eq!(id, parsed_id);

        // test invalid length
        let short_str = "abc";
        let result = short_str.parse::<super::Id>();
        assert!(result.is_err());

        // test invalid characters
        let invalid_str = "invalid_characters_here";
        let result = invalid_str.parse::<super::Id>();
        assert!(result.is_err());
    }
}