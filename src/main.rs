use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::error::Error;
use std::str::FromStr;
use std::time::Duration;
use std::thread;
use tokio::time;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct TempTransPacket {
    temp: f64,
    name: String,
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
    // loop {
    println!("Reading from weather_cat");
    let res = weather_cat.read(cmd_char).await.unwrap();
    let transdata_json = hex_to_str(res)?;
    let trans_data: TempTransPacket = serde_json::from_str(&transdata_json)?;
    println!("{}");
    // thread::sleep(Duration::from_millis(1000))
    // }
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
