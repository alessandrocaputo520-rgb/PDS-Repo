# Relazione G10: verifica requisiti e note di collaudo

**Consegna:** `00 - presentazione Progetto.pdf` (9 pagine); divisione in
`divisione.pdf`. Questo file documenta le funzionalità previste dal codice,
ma non sostituisce l'esecuzione dei test e delle misure sulle macchine reali.

| Requisito delle slide | Codice | Verifica suggerita |
| --- | --- | --- |
| Registrazione account/password | `server/auth.rs`, `storage.rs`, `client/auth.rs` | Crea Alice, prova login corretto e password errata |
| Posizioni simulate | `client/emulator/*` | Avvia client con `file examples/torino_asti.csv` |
| Ogni 30 s invio GPS | `client/main.rs` | Osserva timestamp di posizioni SQLite |
| Stato sconnesso | `server/tracker.rs` | Disconnetti il client e digita `users` sul server |
| Stato movimento al primo cambiamento | `server/tracker.rs` | Prova `manual`, poi `pos LAT LON` |
| Stato fermo dopo 3 minuti | `server/tracker.rs` | Invia posizione invariata per 180 s |
| Tragitto, velocità e tempi | `server/analytics.rs` | `report alice day` dopo un percorso |
| Oggi, settimana, mese | `analytics.rs` | Tre periodi nella console server |
| Broadcast e messaggio diretto | `server/messaging.rs`, `network.rs` | `broadcast`, `to alice ...` |
| Testo client->server | `client/main.rs` | `msg prova` e controlla console server |
| Due piattaforme | CI Ubuntu+macOS | Avvia workflow GitHub Actions su entrambe |
| CPU ogni 2 min in file | `server/logger.rs` | Lascia server acceso almeno 2 minuti |
| Dimensione binario | `scripts/size_report.sh` | Esegui `cargo build --release` e script |

## Procedura di test manuale

1. `cargo fmt --all -- --check`, `cargo test --workspace --all-targets`.
2. Avvia il server, registra 2 account con 2 client in terminali separati.
3. Invia più coordinate da un client (modalità manuale) e verifica stato.
4. Attendi 180 secondi con posizione costante, verifica `Fermo`.
5. Dalla console server esegui `users`, `broadcast TEST`, `to alice TEST`.
6. Dal client invia `msg TEST` e `report day`; verifica report e cronologia.
7. Chiudi/rilancia server e client: il database deve mantenere gli utenti.
8. Dopo almeno 120 s, controlla il log server CPU.
9. Ripeti build/test su Linux e macOS (CI); misura i binari release.

## Misure da rilevare durante il collaudo

| Piattaforma | Binario server (byte) | Binario client (byte) | Test |
| --- | ---: | ---: | --- |
| Linux x86_64 | **da misurare** | **da misurare** | **da eseguire** |
| macOS arm64/x86_64 | **da misurare** | **da misurare** | **da eseguire** |

**Nota:** questi dati non possono essere inventati; i numeri vanno misurati
sulla build `release` sulla macchina destinata alla valutazione.
