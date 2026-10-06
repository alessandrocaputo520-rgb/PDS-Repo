# G10 — Sistema di geolocalizzazione e comunicazione (PDS 2025/26)

Applicazione CLI **client/server in Rust**, coerente con la consegna del corso di
Programmazione di Sistema: utenti autenticati, telemetria GPS simulata ogni **30 s**,
stato veicoli, cronologia su SQLite, interrogazioni e messaggi.

## Prerequisiti

Rust stable / Cargo (`rustup.rs`) su Linux o macOS, accesso iniziale a crates.io
per scaricare le librerie. Il codice usa Tokio, Serde/Bincode, Rusqlite,
Argon2id, Chrono e libc. Le dipendenze di `rusqlite` sono compilate con SQLite
bundled. Test e build in CI sono definiti per **Ubuntu e macOS**.

## Avvio veloce

Aprire **tre terminali** nella cartella principale (workspace).

Terminale 1 (server):

```bash
cargo run --bin server
```

Terminale 2 (primo client):

```bash
cargo run --bin client
```

Nella console client digitare `register`, un nome (`alice`) e una password
di almeno 8 caratteri. Quindi scegliere un emulatore:

```text
file examples/torino_asti.csv
```

Terminale 3 (secondo client):

```bash
cargo run --bin client
```

Creare un secondo account, scegliere ad esempio:

```text
route 45.0618513 7.6606506 44.9084148 8.1778599 5 3
```

Il percorso viene emulato in **5 minuti di movimento**, con **3 minuti di pausa**
al punto intermedio. Da ogni client è possibile inviare messaggi al server:

```text
msg Ciao dal veicolo
report day
report week
report month
quit
```

Lo stato `Fermo` si attiva dopo 3 minuti di coordinate invariate.
Il server registra gli aggiornamenti anche se la posizione non cambia,
necessari a calcolare la durata delle soste.

## Modalità di emulazione

| Sintassi | Descrizione |
| --- | --- |
| `file examples/torino_asti.csv` | Traccia temporizzata, intervalli indicati come `MM:SS;LAT;LON` (supporta anche il formato delle slide) |
| `manual 45.06 7.66` | Coordinate iniziali fisse; comando `pos 45.07 7.67` per cambiarle |
| `random 45.06 7.66` | Random walk con periodi di pausa |
| `route 45 7 44.9 8 5 3` | Interpolazione lineare A->B, 5 min viaggio + 3 min sosta al centro |

Un campione viene spedito subito, poi ogni 30 secondi. Al termine della
traccia file/route il veicolo rimane sull'ultima coordinata.

## Console di amministrazione del server

I seguenti comandi vanno scritti nel **terminale server**, non nel client:

```text
help
users
broadcast Messaggio a tutti gli utenti online
to alice Messaggio per alice
report alice day
report alice week
report alice month
quit
```

Il server stampa anche i messaggi inviati dai client. La cronologia dei
messaggi viene registrata in SQLite e i messaggi diretti inviati a un utente
offline gli vengono mostrati al successivo login. L'analisi indica distanza totale,
tragitto (coordinate, abbreviato quando supera 60 punti), velocità media durante il movimento, secondi di
movimento e pause. I periodi sono in **UTC** e la settimana inizia lunedì.

## Configurazione

Variabili di ambiente (sia server sia client per `PDS_ADDR`):

```bash
PDS_ADDR=127.0.0.1:7878  # indirizzo host:porta TCP
PDS_DB=fleet.db           # file persistente SQLite (solo server)
PDS_CPU_LOG=server_cpu.log # log CPU (solo server)
```

Per client su un'altra macchina della stessa rete, esporre il server
sull'interfaccia `0.0.0.0` e indicare nel client l'IP del server; verificare
firewall e ACL di rete.

**Sicurezza:** le password sono archiviate come hash Argon2id con salt unico,
ma **il TCP usato per il login non è cifrato**. Usare solo loopback/reti fidate
per la dimostrazione: per Internet servirebbe TLS.

## Persistenza

SQLite crea le tabelle `users`, `positions`, `messages` e l'indice
`idx_positions_user_time`; account, coordinate e messaggi sopravvivono al
riavvio del server. Il timestamp GPS lato server evita date arbitrarie
fornite dai client. Campioni non validi vengono rifiutati.

## CPU, dimensione eseguibili, prestazioni

Il server scrive ogni **120 secondi** in `server_cpu.log`: timestamp,
tempo CPU totale in secondi e incremento nei 120 secondi. Il valore è
prelevato dall'orologio CPU del processo (`CLOCK_PROCESS_CPUTIME_ID`):
compatibile con Linux e macOS. Tokio usa I/O non bloccante e timer;
SQLite indicizza per (utente, tempo); i messaggi hanno frame length-prefixed
con limite di **10 MiB**.

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets
cargo build --release --workspace
./scripts/size_report.sh
```

Su Windows, usare `scripts/size_report.ps1` dopo la build. Il logger CPU
nella presente versione è implementato per le due piattaforme richieste,
Linux e macOS; altrove scrive `n/d`.

## Struttura del codice e responsabilità

| Modulo | Coppia | Funzione |
| --- | --- | --- |
| `common/models.rs`, `common/protocol.rs` | Condiviso | Tipi e messaggi |
| `common/geo.rs` | 2 | Haversine / velocità |
| `server/network.rs`, `auth.rs`, `storage.rs`, `logger.rs` | 1 | Infrastruttura |
| `server/tracker.rs`, `analytics.rs`, `messaging.rs` | 2 | Dominio |
| `client/network.rs`, `auth.rs` | 1 | Client base |
| `client/emulator/*`, `messaging.rs` | 2 | Emulazione e ricezione messaggi |

## Limitazioni note

- Applicazione CLI (la consegna non impone una GUI).
- La velocità media è calcolata sui tratti tra **campioni successivi diversi**:
  le pause corrispondono a intervalli con coordinate uguali. Poiché i
  campioni sono discreti, la misura è una stima.
- Alla disconnessione uno stato ritorna `Sconnesso`; al login parte da
  `Fermo` finché non c'è un cambiamento di coordinate.
- Il broadcast raggiunge gli utenti connessi in quel momento.
- Nessuna cifratura del trasporto TCP (vedere sopra).

Fare riferimento a [`REPORT_PROGETTO.md`](REPORT_PROGETTO.md) per la verifica
puntuale dei requisiti e le dimensioni da compilare dopo la build.
