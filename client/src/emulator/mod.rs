pub mod file;
pub mod manual;
pub mod random;
pub mod route;

use self::{file::FileEmulator, manual::Manual, random::Random, route::Route};
use std::io;

#[derive(Debug)]
pub enum Emulator {
    File(FileEmulator),
    Manual(Manual),
    Random(Random),
    Route(Route),
}
impl Emulator {
    pub fn next_position(&mut self) -> (f64, f64) {
        match self {
            Self::File(f) => f.next_position(),
            Self::Manual(m) => m.next_position(),
            Self::Random(r) => r.next_position(),
            Self::Route(r) => r.next_position(),
        }
    }
    pub fn set_manual(&mut self, lat: f64, lon: f64) -> Result<(), &'static str> {
        if !valid_coords(lat, lon) {
            return Err("Coordinate non valide");
        }
        match self {
            Self::Manual(m) => {
                m.position = (lat, lon);
                Ok(())
            }
            _ => Err("Comando pos disponibile solo in modo manual"),
        }
    }
}

pub fn valid_coords(lat: f64, lon: f64) -> bool {
    lat.is_finite()
        && lon.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
}

pub fn parse_mode(line: &str) -> io::Result<Emulator> {
    let args: Vec<_> = line.split_whitespace().collect();
    let invalid = || {
        io::Error::new(io::ErrorKind::InvalidInput,
        "Modalità: file PERCORSO | manual LAT LON | random LAT LON | route LAT1 LON1 LAT2 LON2 MINUTI [PAUSA_MINUTI]")
    };
    if args.is_empty() {
        return Err(invalid());
    }
    let get_coord = |i: usize| args.get(i).and_then(|v| v.parse::<f64>().ok());
    let p = |a: usize, b: usize| -> Option<(f64, f64)> {
        let (lat, lon) = (get_coord(a)?, get_coord(b)?);
        valid_coords(lat, lon).then_some((lat, lon))
    };
    match args[0] {
        "file" if args.len() == 2 => Ok(Emulator::File(FileEmulator::open(args[1])?)),
        "manual" if args.len() == 3 => {
            let (lat, lon) = p(1, 2).ok_or_else(invalid)?;
            Ok(Emulator::Manual(Manual::new(lat, lon)))
        }
        "random" if args.len() == 3 => {
            let (lat, lon) = p(1, 2).ok_or_else(invalid)?;
            Ok(Emulator::Random(Random::new(lat, lon)))
        }
        "route" if args.len() == 6 || args.len() == 7 => {
            let (start, end) = (p(1, 2).ok_or_else(invalid)?, p(3, 4).ok_or_else(invalid)?);
            let minutes = args[5]
                .parse::<u64>()
                .ok()
                .filter(|n| *n > 0 && *n <= 1440)
                .ok_or_else(invalid)?;
            let pause = if args.len() == 7 {
                args[6].parse::<u64>().map_err(|_| invalid())?
            } else {
                0
            };
            if pause > 1440 {
                return Err(invalid());
            }
            Ok(Emulator::Route(Route::new(start, end, minutes, pause)))
        }
        _ => Err(invalid()),
    }
}
