use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::error::Error;
use std::io::prelude::*;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use std::thread;
use tokio::time;
use uuid::Uuid;
use serde::{Deserialize, Serialize, Serializer};
use chrono::serde::ts_seconds;
use rsa::{RsaPrivateKey, Pkcs1v15Encrypt};
use rsa::pkcs1::DecodeRsaPrivateKey;


/// RSA-2048 PKCS#8 private key encoded as PEM
const PRIV_PEM: &str = include_str!("../private_key.test.pem");

pub fn serialize_dt<S>(
    dt: &Option<chrono::DateTime<chrono::Utc>>, 
    serializer: S
) -> Result<S::Ok, S::Error> 
where
    S: Serializer {
    match dt {
        Some(dt) => ts_seconds::serialize(dt, serializer),
        _ => unreachable!(),
    }
}

#[derive(Deserialize, Debug)]
struct TempTransPacket {
    pub t: f64,
    pub h: f64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TempratureStore {
    pub temp: f64,
    pub hum: f64,
    pub name: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub time_stamp: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await.unwrap();

    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().nth(0).unwrap();

    central.start_scan(ScanFilter::default()).await?;
    time::sleep(Duration::from_secs(2)).await;

    // find the device we're interested in
    let mut weather_cat_opt = find_weather_cat(&central).await;
    while weather_cat_opt.is_none() {
        weather_cat_opt = find_weather_cat(&central).await;
        central.start_scan(ScanFilter::default()).await?;
    }
    let weather_cat = weather_cat_opt.unwrap();

    weather_cat.connect().await?;
    weather_cat.discover_services().await?;

    let chars = weather_cat.characteristics();
    chars.iter().for_each(|c| println!("{}", c));
    let cmd_char = chars.iter().find(|c| c.uuid == Uuid::from_str("00002a6e-0000-1000-8000-00805f9b34fb").unwrap()).unwrap();

    thread::sleep(Duration::from_millis(1000));
    println!("Reading from weather_cat");
    let res = weather_cat.read(cmd_char).await.unwrap();
    // let transdata_json = decode_rsa(hex_to_str(res)?);
    let transdata_json = hex_to_str(res)?;
    println!("transdata_json {}",transdata_json);
    let trans_data: TempTransPacket = serde_json::from_str(&transdata_json)?;
    let data = TempratureStore {
        temp: trans_data.t,
        hum: trans_data.h,
        name: trans_data.name,
        time_stamp: chrono::Utc::now(),
    };
    println!("{:?}", data);
    save_to_file(data, "./temp_store.json".to_string())?;

    return Ok(());
}

fn save_to_file(d: TempratureStore, file_path: String) -> Result<(), std::io::Error>{
    let mut file = std::fs::File::open(&file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let mut temp_store: Vec<TempratureStore> = serde_json::from_str(&contents)?;
    temp_store.push(d);

    let j = serde_json::to_string(&temp_store)?;

    let mut f = std::fs::OpenOptions::new().write(true).open(file_path)?;
    f.write_all(j.as_bytes())?;
    f.flush()?;

    return Ok(());
}

/// parse the ascii string from the vec u8 bytes
fn hex_to_str(bytes: Vec<u8>) -> Result<String, std::io::Error> {
    if bytes.len() % 2 != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid hex string"));
    }
    return match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to convert bytes to string")),
    };
}

async fn find_weather_cat(central: &Adapter) -> Option<Peripheral> {
    for p in central.peripherals().await.unwrap() {
        println!("{:?}", p.properties().await.unwrap().unwrap().local_name);
        if p.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|name| name.contains("WeatherCat"))
        {
            return Some(p);
        }
    }
    None
}
