//! Importa CSV semicolon/comma delimitato o esempio slide con spazi e virgole decimali.
use std::{fs, io};

#[derive(Debug)]
pub struct FileEmulator {
    samples: Vec<(u64, f64, f64)>,
    elapsed: u64,
    last: usize,
}
impl FileEmulator {
    pub fn open(path: &str) -> io::Result<Self> {
        let raw = fs::read_to_string(path)?;
        let samples = parse(&raw)?;
        Ok(Self {
            samples,
            elapsed: 0,
            last: 0,
        })
    }
    pub fn next_position(&mut self) -> (f64, f64) {
        while self.last + 1 < self.samples.len() && self.samples[self.last + 1].0 <= self.elapsed {
            self.last += 1;
        }
        let (_, lat, lon) = self.samples[self.last];
        self.elapsed += 30;
        (lat, lon)
    }
}

fn parse(raw: &str) -> io::Result<Vec<(u64, f64, f64)>> {
    let mut rows = Vec::new();
    for (line_index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.to_lowercase().starts_with("time")
        {
            continue;
        }
        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').map(str::trim).collect()
        } else if trimmed.split_whitespace().count() == 3 {
            trimmed.split_whitespace().collect()
        } else {
            trimmed.split(',').map(str::trim).collect()
        };
        if parts.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Riga {}: attesi tempo, lat, lon", line_index + 1),
            ));
        }
        let secs = parse_clock(parts[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Tempo non valido"))?;
        let number = |v: &str| v.replace(',', ".").parse::<f64>().ok();
        let lat = number(parts[1])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Latitudine non valida"))?;
        let lon = number(parts[2])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Longitudine non valida"))?;
        if !super::valid_coords(lat, lon) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GPS fuori range",
            ));
        }
        if rows
            .last()
            .is_some_and(|(t, _, _): &(u64, f64, f64)| *t >= secs)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Timestamp non ordinati",
            ));
        }
        rows.push((secs, lat, lon));
    }
    if rows.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "File GPS vuoto"));
    }
    let start = rows[0].0;
    for row in &mut rows {
        row.0 -= start;
    }
    Ok(rows)
}
fn parse_clock(s: &str) -> Option<u64> {
    let parts: Vec<_> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let mins: u64 = parts[0].parse().ok()?;
    let secs: u64 = parts[1].parse().ok()?;
    if secs >= 60 {
        return None;
    }
    mins.checked_mul(60)?.checked_add(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slide_format_and_csv() {
        let rows = parse("00:00 45,0618513 7,6606506\n00:30 45,0575226 7,6618322").unwrap();
        assert_eq!(rows.len(), 2);
        assert!((rows[0].1 - 45.0618513).abs() < 0.0000001);
        assert_eq!(parse("time;lat;lon\n00:00;45.0;7.0").unwrap().len(), 1);
        assert!(parse("00:30,45,7\n00:00,45,7").is_err());
    }
}
