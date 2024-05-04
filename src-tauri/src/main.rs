// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use btleplug::api::{
    bleuuid::uuid_from_u16, Central, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use futures::StreamExt;
use btleplug::platform::{Adapter, Manager, Peripheral};
use serde::{Deserialize, Serialize};
use std::{fs, sync::{Arc, Mutex}, time::Duration};
use tokio::time;
use uuid::Uuid;

struct AppState {
    central: Adapter,
    treadmill: Option<Arc<Mutex<Peripheral>>>,
}

const TREADMILL_DATA_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0x2ACD);
const TREADMILL_CONTROL_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0x2AD9);

#[derive(Debug, Serialize, Deserialize)]
struct TreadmillDataFlags {
    more_data: bool,
    average_speed: bool,
    total_distance: bool,
    inclination_and_ramp_angle: bool,
    elevation_gain: bool,
    instantaneous_pace: bool,
    average_pace: bool,
    energy: bool,
    heart_rate: bool,
    metabolic_equivalent: bool,
    elapsed_time: bool,
    remaining_time: bool,
    force_on_belt_and_power_output: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct TreadmillData {
    speed: u16,
    average_speed: Option<u16>,
    total_distance: Option<u32>,
    inclination: Option<i16>,
    ramp_angle: Option<i16>,
    positive_elevation: Option<u16>,
    negative_elevation: Option<u16>,
    instantaneous_pace: Option<u16>,
    average_pace: Option<u16>,
    total_energy: Option<u16>,
    energy_per_hour: Option<u16>,
    energy_per_minute: Option<u8>,
    heart_rate: Option<u8>,
    metabolic_equivalent: Option<u8>,
    elapsed_time: Option<u16>,
    remaining_time: Option<u16>,
    force_on_belt: Option<i16>,
    power_output: Option<i16>,
}

enum DecodeError {
    NotEnoughData,
}

enum TreadmillCommands {
    RequestControl,
    Reset,
    SetTargetSpeed(u16),
    SetTargetInclination(i16),
    StartOrResume,
    StopOrPause,
    // TODO: This is a u24, not a u32
    SetTargetedDistance(u32),
    SetTargetedTrainingTime(u16),
}

// Decoding based on https://github.com/oesmith/gatt-xml/blob/master/org.bluetooth.characteristic.treadmill_data.xml
fn decode_treadmill_data(data: &[u8]) -> Result<TreadmillData, DecodeError> {
    if data.len() < 4 {
        return Err(DecodeError::NotEnoughData);
    }

    let flags = TreadmillDataFlags {
        more_data: data[0] & 0b00000001 != 0,
        average_speed: data[0] & 0b00000010 != 0,
        total_distance: data[0] & 0b00000100 != 0,
        inclination_and_ramp_angle: data[0] & 0b00001000 != 0,
        elevation_gain: data[0] & 0b00010000 != 0,
        instantaneous_pace: data[0] & 0b00100000 != 0,
        average_pace: data[0] & 0b01000000 != 0,
        energy: data[0] & 0b10000000 != 0,
        heart_rate: data[1] & 0b00000001 != 0,
        metabolic_equivalent: data[1] & 0b00000010 != 0,
        elapsed_time: data[1] & 0b00000100 != 0,
        remaining_time: data[1] & 0b00001000 != 0,
        force_on_belt_and_power_output: data[1] & 0b00010000 != 0,
    };
    let speed = u16::from_le_bytes([data[2], data[3]]);
    let mut cursor = 4;

    let mut average_speed = None;
    if flags.average_speed {
        if data.len() < cursor + 2 {
            return Err(DecodeError::NotEnoughData);
        }
        average_speed = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        cursor += 2;
    }

    let mut total_distance = None;
    if flags.total_distance {
        if data.len() < cursor + 3 {
            return Err(DecodeError::NotEnoughData);
        }
        // TODO: 0 might be in the wrong place, this is a u24...
        total_distance = Some(u32::from_le_bytes([data[cursor], data[cursor + 1], data[cursor + 2], 0]));
        cursor += 3;
    }

    let mut inclination = None;
    let mut ramp_angle = None;
    if flags.inclination_and_ramp_angle {
        if data.len() < cursor + 4 {
            return Err(DecodeError::NotEnoughData);
        }
        inclination = Some(i16::from_le_bytes([data[cursor], data[cursor + 1]]));
        ramp_angle = Some(i16::from_le_bytes([data[cursor+ 2], data[cursor + 3]]));
        cursor += 4;
    }

    let mut positive_elevation = None;
    let mut negative_elevation = None;
    if flags.elevation_gain {
        if data.len() < cursor + 4 {
            return Err(DecodeError::NotEnoughData);
        }
        positive_elevation = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        negative_elevation = Some(u16::from_le_bytes([data[cursor + 2], data[cursor + 3]]));
        cursor += 4;
    }

    let mut instantaneous_pace = None;
    if flags.instantaneous_pace {
        if data.len() < cursor + 2 {
            return Err(DecodeError::NotEnoughData);
        }
        instantaneous_pace = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        cursor += 2;
    }

    let mut average_pace = None;
    if flags.average_pace {
        if data.len() < cursor + 2 {
            return Err(DecodeError::NotEnoughData);
        }
        average_pace = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        cursor += 2;
    }

    let mut total_energy = None;
    let mut energy_per_hour = None;
    let mut energy_per_minute = None;
    if flags.energy {
        if data.len() < cursor + 5 {
            return Err(DecodeError::NotEnoughData);
        }
        total_energy = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        energy_per_hour = Some(u16::from_le_bytes([data[cursor + 2], data[cursor + 3]]));
        energy_per_minute = Some(u8::from_le_bytes([data[cursor + 4]]));
        cursor += 5;
    }

    let mut heart_rate = None;
    if flags.heart_rate {
        if data.len() < cursor + 1 {
            return Err(DecodeError::NotEnoughData);
        }
        heart_rate = Some(data[cursor]);
        cursor += 1;
    }

    let mut metabolic_equivalent = None;
    if flags.metabolic_equivalent {
        if data.len() < cursor + 1 {
            return Err(DecodeError::NotEnoughData);
        }
        metabolic_equivalent = Some(u8::from_le_bytes([data[cursor]]));
        cursor += 1;
    }

    let mut elapsed_time = None;
    if flags.elapsed_time {
        if data.len() < cursor + 2 {
            return Err(DecodeError::NotEnoughData);
        }
        elapsed_time = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        cursor += 2;
    }

    let mut remaining_time = None;
    if flags.remaining_time {
        if data.len() < cursor + 2 {
            return Err(DecodeError::NotEnoughData);
        }
        remaining_time = Some(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        cursor += 2;
    }

    let mut force_on_belt = None;
    let mut power_output = None;
    if flags.force_on_belt_and_power_output {
        if data.len() < cursor + 4 {
            return Err(DecodeError::NotEnoughData);
        }
        force_on_belt = Some(i16::from_le_bytes([data[cursor], data[cursor + 1]]));
        power_output = Some(i16::from_le_bytes([data[cursor + 2], data[cursor + 3]]));
    }

    Ok(TreadmillData {
        speed,
        average_speed,
        total_distance,
        inclination,
        ramp_angle,
        positive_elevation,
        negative_elevation,
        instantaneous_pace,
        average_pace,
        total_energy,
        energy_per_hour,
        energy_per_minute,
        heart_rate,
        metabolic_equivalent,
        elapsed_time,
        remaining_time,
        force_on_belt,
        power_output,
    })
}

fn treadmill_command_to_message(command: TreadmillCommands) -> Vec<u8> {
    match command {
        TreadmillCommands::RequestControl => vec![0x00],
        TreadmillCommands::Reset => vec![0x01],
        TreadmillCommands::SetTargetSpeed(speed) => vec![0x02, speed.to_le_bytes()[0], speed.to_le_bytes()[1]],
        TreadmillCommands::SetTargetInclination(inclination) => vec![0x03, inclination.to_le_bytes()[0], inclination.to_le_bytes()[1]],
        TreadmillCommands::StartOrResume => vec![0x07],
        TreadmillCommands::StopOrPause => vec![0x08],
        TreadmillCommands::SetTargetedDistance(distance) => vec![0x0C, distance.to_le_bytes()[0], distance.to_le_bytes()[1], distance.to_le_bytes()[2]],
        TreadmillCommands::SetTargetedTrainingTime(time) => vec![0x0D, time.to_le_bytes()[0], time.to_le_bytes()[1]],
    }
}

async fn find_treadmill(central: &Adapter) -> Option<Peripheral> {
    let peripherals = match central.peripherals().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error discovering peripherals: {:?}", e);
            return None
        }
    };

    for p in peripherals {
        if p.properties().await.unwrap().unwrap().local_name.iter().any(|name| name.contains("HORIZON_7.0AT")) {
            return Some(p);
        }
    }

    return None;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "unit", content = "value")]
enum PaceRaw {
    #[serde(rename = "mph")]
    MPH(String),
    #[serde(rename = "kph")]
    KPH(String),
    #[serde(rename = "min/mi")]
    MinPerMi(String),
    #[serde(rename = "min/km")]
    MinPerKm(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WorkoutStepRaw {
    #[serde(rename = "repeat")]
    Repeat {
        times: u8,
        steps: Vec<WorkoutStepRaw>,
    },
    #[serde(rename = "run")]
    Run {
        name: String,
        duration: String,
        pace: PaceRaw,
        angle: i16
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkoutRaw {
    name: String,
    description: String,
    steps: Vec<WorkoutStepRaw>,
}

#[derive(Debug, Serialize, Clone)]
struct WorkoutStep {
    name: String,
    duration: u16,
    distance: u16,
    // Km/h at 0.01 precision
    pace: u16,
    angle: i16,
}

#[derive(Debug, Serialize)]
struct Workout {
    duration: u16,
    distance: u16,
    steps: Vec<WorkoutStep>,
    name: String,
    description: String,
}

fn parse_pace(pace: &PaceRaw) -> u16 {
    match pace {
        PaceRaw::MinPerMi(value) => {
            let parts = value.split(":").collect::<Vec<_>>();
            let minutes = parts.get(0).unwrap_or(&"0").parse::<u16>().unwrap();
            let seconds = parts.get(1).unwrap_or(&"0").parse::<u16>().unwrap();
            let seconds_per_mile = (minutes * 60 + seconds) as f64;
            let km_per_hour = 1. / seconds_per_mile * (60.0 * 60.0) * (1.60934/1.);
            println!("Seconds per mile: {:?}", seconds_per_mile as u16);
            println!("Km per hour: {:?}", (km_per_hour * 100.) as u16);
            (km_per_hour * 100.) as u16
        }
        _ => {
            0
        }
    }
}

fn parse_duration(pace: &str) -> u16 {
    let parts = pace.split(":").collect::<Vec<_>>();
    let minutes = parts.get(0).unwrap_or(&"0").parse::<u16>().unwrap();
    let seconds = parts.get(1).unwrap_or(&"0").parse::<u16>().unwrap();
    minutes * 60 + seconds
}

fn parse_workout_step(step: &WorkoutStepRaw) -> Vec<WorkoutStep> {
    match step {
        WorkoutStepRaw::Repeat { times, steps } => {
            let steps = steps.iter().flat_map(|s| parse_workout_step(s)).collect::<Vec<_>>();
            let mut result = Vec::new();
            for _ in 0..*times {
                result.extend(steps.clone());
            }
            result
        },
        WorkoutStepRaw::Run { name, duration, pace, angle } => {
            let pace = parse_pace(pace);
            let duration = parse_duration(duration);
            let distance = (pace as f32 * duration as f32 / 1000.0) as u16;
            vec![WorkoutStep {
                name: name.clone(),
                duration,
                distance,
                pace,
                angle: *angle,
            }]
        }
    }
}

fn parse_workout(workout: &WorkoutRaw) -> Workout {
    let steps = workout.steps.iter().flat_map(|s| parse_workout_step(s)).collect::<Vec<_>>();
    let mut distance = 0;
    let mut duration = 0;
    for step in &steps {
        distance += step.distance;
        duration += step.duration;
    }

    Workout {
        duration,
        distance,
        steps,
        name: workout.name.clone(),
        description: workout.description.clone(),
    }
}

#[tauri::command]
fn read_workouts() -> Result<Vec<String>, String> {
    let paths = match fs::read_dir("/Users/kyle/Projects/run/workouts") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error reading workouts directory: {:?}", e);
            return Err("Error reading workouts directory.".to_string());
        }
    };

    let mut workouts = Vec::new();
    for path in paths {
        let path = path.unwrap();
        workouts.push(path.file_name().into_string().unwrap());

        let file = fs::read_to_string(path.path());
        let content = match file {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error reading file: {:?}", e);
                return Err("Error reading file.".to_string());
            }
        };
        let workout: WorkoutRaw = match serde_json::from_str(&content) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Error parsing JSON: {:?}", e);
                return Err("Error parsing JSON.".to_string());
            }
        };
        println!("Raw Workout {:?}", workout);
        let parsed_workout = parse_workout(&workout);
        println!("Parsed Workout {:?}", parsed_workout);
    }

    Ok(workouts)
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn connect_to_treadmill(name: String) -> Result<String, String> {
    let manager = Manager::new().await.unwrap();

    let central = manager
        .adapters()
        .await
        .expect("Unable to fetch adapter list.")
        .into_iter()
        .nth(0)
        .expect("Unable to find adapters.");

    match central.start_scan(ScanFilter::default()).await {
        Ok(_) => println!("Scanning for devices..."),
        Err(e) => eprintln!("Error scanning: {:?}", e),
    }

    time::sleep(Duration::from_secs(2)).await;

    let treadmill = match find_treadmill(&central).await {
        Some(p) => p,
        None => {
            eprintln!("Treadmill not found.");
            return Ok("Treadmill not found.".to_string());
        }
    };

    match treadmill.connect().await {
        Ok(_) => println!("Connected to treadmill."),
        Err(e) => {
            eprintln!("Error connecting to treadmill: {:?}", e);
            return Ok("Error connecting to treadmill.".to_string());
        }
    }

    treadmill.discover_services().await.unwrap();

    let characteristics = treadmill.characteristics();
    let char = characteristics.iter().find(|c| c.uuid == TREADMILL_DATA_CHARACTERISTIC_UUID).unwrap();
    treadmill.subscribe(char).await.unwrap();

    let mut sub = treadmill.notifications().await.unwrap();
    tokio::spawn(async move {
        while let Some(notification) = sub.next().await {
            match decode_treadmill_data(&notification.value) {
                Ok(data) => {
                    println!("Data: {:?}", data);
                },
                Err(_) => {
                    println!("Error decoding data.");
                }
            }
            println!("Notification: {:?}", notification);
        }
    });

    time::sleep(Duration::from_secs(5)).await;

    let control_char = characteristics.iter().find(|c| c.uuid == TREADMILL_CONTROL_CHARACTERISTIC_UUID).unwrap();
    treadmill.write(control_char, &treadmill_command_to_message(TreadmillCommands::RequestControl), WriteType::WithoutResponse).await.unwrap();
    time::sleep(Duration::from_secs(5)).await;
    treadmill.write(control_char, &treadmill_command_to_message(TreadmillCommands::StartOrResume), WriteType::WithoutResponse).await.unwrap();
    treadmill.write(control_char, &treadmill_command_to_message(TreadmillCommands::SetTargetSpeed(200)), WriteType::WithoutResponse).await.unwrap();

    Ok(format!("Hello, {}! You've been greeted from Rust!", name))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![connect_to_treadmill, read_workouts])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
