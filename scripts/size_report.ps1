# Powershell / Windows: cargo build --release --workspace
foreach ($app in @('server', 'client')) {
    $file = "target/release/$app.exe"
    if (!(Test-Path $file)) { throw "File mancante: $file" }
    $bytes = (Get-Item $file).Length
    Write-Output ("{0}: {1} byte ({2:N2} MiB)" -f $file, $bytes, ($bytes / 1MB))
}
