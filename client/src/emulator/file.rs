use super::Emulator;
use std::{fs::File, path::Path};

/// Emulatore che legge le posizioni da un file CSV.
///
/// Formati accettati (con intestazione opzionale):
/// - `time_secs,lat,lon`     — offset in secondi dall'inizio
/// - `mm:ss,lat,lon`         — minuti:secondi
/// - `HH:MM:SS,lat,lon`      — ore:minuti:secondi
///
/// Ogni riga rappresenta una posizione inviata al tick successivo (circa ogni 30 secondi).
pub struct FileEmulator {
    positions: Vec<(f64, f64)>,
    index: usize,
}

impl FileEmulator {
    /// Carica il file CSV dal percorso indicato.
    pub fn new(path: &str) -> Result<Self, String> {
        if !Path::new(path).exists() {
            return Err(format!("File non trovato: {}", path));
        }

        let file = File::open(path).map_err(|e| format!("Errore apertura file: {e}"))?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(file);

        let mut positions = Vec::new();

        for result in rdr.records() {
            let record = result.map_err(|e| format!("Errore parsing CSV: {e}"))?;

            if record.len() < 3 {
                continue;
            }

            // Le colonne lat e lon sono sempre le ultime due
            let lat_str = record.get(record.len() - 2).unwrap_or("").replace(',', ".");
            let lon_str = record.get(record.len() - 1).unwrap_or("").replace(',', ".");

            let lat: f64 = match lat_str.trim().parse() {
                Ok(v) => v,
                Err(_) => continue, // salta righe non numeriche (es. intestazioni)
            };
            let lon: f64 = match lon_str.trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            positions.push((lat, lon));
        }

        if positions.is_empty() {
            return Err("Il file CSV non contiene posizioni valide.".to_string());
        }

        println!(
            "FileEmulator: caricati {} punti GPS da '{}'",
            positions.len(),
            path
        );
        Ok(Self { positions, index: 0 })
    }

    pub fn remaining(&self) -> usize {
        self.positions.len().saturating_sub(self.index)
    }
}

impl Emulator for FileEmulator {
    fn next_position(&mut self) -> Option<(f64, f64)> {
        if self.index < self.positions.len() {
            let pos = self.positions[self.index];
            self.index += 1;
            Some(pos)
        } else {
            None // percorso terminato
        }
    }

    fn description(&self) -> &str {
        "Lettura da file CSV"
    }
}
